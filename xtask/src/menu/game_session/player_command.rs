use super::{closed, count, player_child};
use anyhow::{Result, ensure};
use serde_json::Value;
fn bounded(value: &Value, maximum: u64) -> bool {
    value.as_u64().is_some_and(|value| value <= maximum)
}
pub(super) fn validate(state: &Value, action: &Value) -> Result<()> {
    if state.is_null() {
        ensure!(
            action["stage"] == "tp_setter_tail_pending",
            "TP command body missing before selector return"
        );
        return Ok(());
    }
    closed(
        state,
        &[
            "stage",
            "opcode",
            "set_reference",
            "set_index",
            "set_groups",
            "groups_visited",
            "weight_writes",
            "pending_group",
            "pending_slot",
            "child_reference",
            "factory_class",
            "child_flags",
            "child_divisor",
            "mask",
            "children_created",
            "child_header",
            "command_returned",
            "cleanup_complete",
            "selector_acknowledged",
            "state_sha256",
            "layers_state_sha256",
        ],
    )?;
    let opcode = count(state, "opcode")?;
    let groups = count(state, "groups_visited")?;
    ensure!(
        action["stage"] == "tp_command_pending"
            && state["opcode"] == action["tp_selector"]["pending_opcode"]
            && opcode <= u64::from(u32::MAX)
            && groups <= 4096
            && count(state, "weight_writes")? <= groups
            && bounded(&state["children_created"], 1)
            && state["command_returned"] == false
            && state["cleanup_complete"] == false
            && state["selector_acknowledged"] == false
            && ["state_sha256", "layers_state_sha256"]
                .into_iter()
                .all(
                    |key| state[key].as_str().is_some_and(|hash| hash.len() == 64
                        && hash
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
                ),
        "TP command prefix claims a completed effect or differs from its selector"
    );
    let no_child = [
        "pending_group",
        "pending_slot",
        "child_reference",
        "factory_class",
        "child_flags",
        "child_divisor",
    ]
    .into_iter()
    .all(|key| state[key].is_null());
    let no_set = ["set_reference", "set_index", "set_groups"]
        .into_iter()
        .all(|key| state[key].is_null());
    match state["stage"].as_str() {
        Some(
            "child_factory_pending"
            | "child_initializer_pending"
            | "child_effects_pending"
            | "cleanup_pending",
        ) => {
            ensure!(
                matches!(opcode, 0x0c00_0002 | 0x0c00_0003)
                    && bounded(&state["set_reference"], u64::from(u32::MAX))
                    && bounded(&state["set_index"], 32766)
                    && bounded(&state["set_groups"], u64::from(u32::MAX))
                    && groups <= count(state, "set_groups")?
                    && state["mask"].is_null(),
                "TP set binding or reached groups differ"
            );
            if state["stage"] != "cleanup_pending" {
                ensure!(
                    groups > 0
                        && state["pending_group"] == groups - 1
                        && bounded(&state["pending_slot"], 3)
                        && bounded(&state["child_reference"], u64::from(u32::MAX))
                        && state["factory_class"]
                            == ((count(state, "child_reference")? >> 24) & 0x7f)
                        && bounded(&state["child_flags"], 65535)
                        && bounded(&state["child_divisor"], u64::from(u32::MAX)),
                    "TP factory request is absent or contradictory"
                );
                if state["stage"] == "child_factory_pending" {
                    ensure!(
                        state["factory_class"] != 4
                            && state["children_created"] == 0
                            && state["child_header"].is_null(),
                        "supported class04 allocation was skipped"
                    );
                } else {
                    ensure!(
                        state["factory_class"] == 4 && state["children_created"] == 1,
                        "retained child header is absent or wrong class"
                    );
                    player_child::validate(&state["child_header"], &state["stage"])?;
                }
            } else {
                ensure!(
                    no_child
                        && groups == count(state, "set_groups")?
                        && state["children_created"] == 0
                        && state["child_header"].is_null(),
                    "TP cleanup skipped an unfinished group or factory"
                );
            }
        }
        Some("mask_control_pending" | "mask_clear_pending") => ensure!(
            opcode == 0x0c00_0005
                && state["children_created"] == 0
                && state["child_header"].is_null()
                && no_set
                && no_child
                && groups == 0
                && bounded(&state["mask"], 65535)
                && (state["mask"] == 0) == (state["stage"] == "mask_clear_pending"),
            "TP mask dependency differs"
        ),
        Some("opcode_pending") => ensure!(
            !matches!(opcode, 0x0c00_0002 | 0x0c00_0003 | 0x0c00_0005)
                && state["children_created"] == 0
                && state["child_header"].is_null()
                && no_set
                && no_child
                && groups == 0
                && state["mask"].is_null(),
            "TP pending opcode falsely claims parsed operands"
        ),
        _ => anyhow::bail!("TP command effect boundary is unsupported"),
    }
    Ok(())
}
#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;
    pub(in super::super) fn fixture() -> Value {
        json!({"stage":"child_initializer_pending","opcode":0x0c00_0002,"set_reference":0x0a00_00ab,"set_index":0,"set_groups":1,
            "groups_visited":1,"weight_writes":1,"pending_group":0,"pending_slot":1,"child_reference":0x0400_00ab,"factory_class":4,
            "child_flags":32,"child_divisor":3,"mask":null,"children_created":1,"child_header":player_child::tests::fixture(),"command_returned":false,"cleanup_complete":false,
            "selector_acknowledged":false,"state_sha256":"a".repeat(64),"layers_state_sha256":"b".repeat(64)})
    }
    #[test]
    fn rejects_missing_prefix_or_unreached_effects_and_private_payload() {
        let action = super::super::player_action::tests::fixture();
        validate(&fixture(), &action).unwrap();
        for (key, value) in [
            ("stage", json!("complete")),
            ("opcode", json!(0)),
            ("set_reference", Value::Null),
            ("set_index", json!(32767)),
            ("set_groups", json!(0)),
            ("groups_visited", json!(2)),
            ("weight_writes", json!(2)),
            ("pending_group", json!(1)),
            ("pending_slot", json!(4)),
            ("child_reference", Value::Null),
            ("factory_class", json!(3)),
            ("child_flags", json!(65536)),
            ("child_divisor", json!(-1)),
            ("mask", json!(1)),
            ("children_created", json!(0)),
            ("command_returned", json!(true)),
            ("cleanup_complete", json!(true)),
            ("selector_acknowledged", json!(true)),
            ("state_sha256", json!("short")),
            ("layers_state_sha256", json!("short")),
            ("payload", json!([1, 2, 3])),
        ] {
            let mut state = fixture();
            state[key] = value;
            assert!(validate(&state, &action).is_err(), "accepted {key}");
        }
        for key in fixture().as_object().unwrap().keys() {
            let mut state = fixture();
            state.as_object_mut().unwrap().remove(key);
            assert!(validate(&state, &action).is_err(), "accepted missing {key}");
        }
        assert!(validate(&Value::Null, &action).is_err());
    }
    #[test]
    fn cleanup_mask_and_unhandled_opcode_remain_separate_dependencies() {
        let mut action = super::super::player_action::tests::fixture();
        let mut state = fixture();
        for key in [
            "pending_group",
            "pending_slot",
            "child_reference",
            "factory_class",
            "child_flags",
            "child_divisor",
        ] {
            state[key] = Value::Null;
        }
        state["stage"] = json!("cleanup_pending");
        state["children_created"] = json!(0);
        state["child_header"] = Value::Null;
        validate(&state, &action).unwrap();
        for key in ["set_reference", "set_index", "set_groups"] {
            state[key] = Value::Null;
        }
        state["groups_visited"] = json!(0);
        state["weight_writes"] = json!(0);
        for (stage, opcode, mask) in [
            ("mask_control_pending", 0x0c00_0005, json!(15)),
            ("mask_clear_pending", 0x0c00_0005, json!(0)),
            ("opcode_pending", 0x0c00_00ff, Value::Null),
        ] {
            state["stage"] = json!(stage);
            state["opcode"] = json!(opcode);
            state["mask"] = mask;
            action["tp_selector"]["pending_opcode"] = json!(opcode);
            validate(&state, &action).unwrap();
        }
        action["stage"] = json!("tp_setter_tail_pending");
        assert!(validate(&state, &action).is_err());
        validate(&Value::Null, &action).unwrap();
    }
}
