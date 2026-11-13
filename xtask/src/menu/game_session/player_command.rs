use super::{closed, count, player_child, player_mask};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
fn bounded(v: &Value, max: u64) -> bool {
    v.as_u64().is_some_and(|n| n <= max)
}
pub(super) fn validate(s: &Value, a: &Value) -> Result<()> {
    let pending = a["stage"] == "tp_command_pending";
    ensure!(
        pending
            || matches!(
                a["stage"].as_str(),
                Some(
                    "tp_setter_tail_pending"
                        | "tp_dispatch_pending"
                        | "fp_dispatch_pending"
                        | "fp_command_pending"
                        | "fp_setter_tail_pending"
                        | "fp_dispatch_tail_pending"
                        | "initial_action_suffix_pending"
                        | "action_dispatch_repeat_pending"
                )
            ),
        "TP command parent stage differs"
    );
    validate_for(s, &a["tp_selector"], pending)
}
pub(super) fn validate_for(s: &Value, selector: &Value, pending: bool) -> Result<()> {
    if s.is_null() {
        ensure!(
            !pending && selector["commands_executed"] == 0,
            "command receipt missing after execution"
        );
        return Ok(());
    }
    closed(
        s,
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
            "mask_effects",
            "children_created",
            "children_completed",
            "children_inserted",
            "head_copies",
            "active_slots",
            "effective_flags_or",
            "child_header",
            "last_completed_child",
            "command_returned",
            "cleanup_complete",
            "cleanup_skipped",
            "selector_acknowledged",
            "state_sha256",
            "layers_state_sha256",
        ],
    )?;
    let op = count(s, "opcode")?;
    let groups = count(s, "groups_visited")?;
    let created = count(s, "children_created")?;
    let completed = count(s, "children_completed")?;
    let returned = s["stage"] == "command_returned";
    ensure!(
        op <= u64::from(u32::MAX)
            && groups <= 4096
            && count(s, "weight_writes")? <= groups
            && created <= 4096
            && completed <= created
            && count(s, "children_inserted")? == completed
            && count(s, "head_copies")? <= created
            && (1..=4).contains(&count(s, "active_slots")?)
            && bounded(&s["effective_flags_or"], 65535)
            && (count(s, "head_copies")? == 0 || count(s, "effective_flags_or")? & 4 != 0)
            && s["command_returned"] == returned
            && s["cleanup_complete"] == false
            && s["cleanup_skipped"] == (returned && count(s, "effective_flags_or")? & 32 != 0)
            && s["selector_acknowledged"].is_boolean()
            && ["state_sha256", "layers_state_sha256"]
                .iter()
                .all(|k| s[k].as_str().is_some_and(|h| h.len() == 64
                    && h.bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)))),
        "command counts/return evidence differs"
    );
    if s["selector_acknowledged"] == true {
        ensure!(
            returned && !pending && count(selector, "commands_executed")? > 0,
            "command acknowledgement differs"
        );
    } else {
        ensure!(
            pending && s["opcode"] == selector["pending_opcode"],
            "pending command differs from selector"
        );
    }
    if completed == 0 {
        ensure!(
            s["last_completed_child"].is_null(),
            "completed child invented"
        );
    } else {
        player_child::validate(&s["last_completed_child"], &json!("child_completed"))?;
        ensure!(
            count(&s["last_completed_child"], "slot")? < count(s, "active_slots")?
                && (s["last_completed_child"]["parent_phase_copied"] != true
                    || count(s, "head_copies")? > 0),
            "completed child slot/copy differs"
        );
    }
    let no_child = [
        "pending_group",
        "pending_slot",
        "child_reference",
        "factory_class",
        "child_flags",
        "child_divisor",
    ]
    .iter()
    .all(|k| s[k].is_null());
    let no_set = ["set_reference", "set_index", "set_groups"]
        .iter()
        .all(|k| s[k].is_null());
    if op == 0x0c00_0005 {
        ensure!(
            created == 0
                && completed == 0
                && s["head_copies"] == 0
                && s["child_header"].is_null()
                && no_set
                && no_child
                && groups == 0
                && s["weight_writes"] == 0
                && s["effective_flags_or"] == 0
                && s["cleanup_complete"] == false
                && s["cleanup_skipped"] == false
                && bounded(&s["mask"], 65535),
            "mask command invented set/child effects"
        );
        return player_mask::validate(
            &s["mask_effects"],
            s["stage"].as_str().unwrap_or(""),
            count(s, "mask")?,
            count(s, "active_slots")?,
        );
    }
    ensure!(
        s["mask_effects"].is_null(),
        "non-mask command owns mask effects"
    );
    match s["stage"].as_str() {
        Some(
            "child_factory_pending"
            | "child_initializer_pending"
            | "child_effects_pending"
            | "child_wrapper_pending"
            | "child_continuation_pending"
            | "cleanup_pending"
            | "command_returned",
        ) => {
            ensure!(
                matches!(op, 0x0c00_0002 | 0x0c00_0003)
                    && bounded(&s["set_reference"], u64::from(u32::MAX))
                    && bounded(&s["set_index"], 32766)
                    && bounded(&s["set_groups"], u64::from(u32::MAX))
                    && groups <= count(s, "set_groups")?
                    && s["mask"].is_null(),
                "set binding differs"
            );
            if returned || s["stage"] == "cleanup_pending" {
                ensure!(
                    no_child
                        && groups == count(s, "set_groups")?
                        && created == completed
                        && s["child_header"].is_null(),
                    "set return skips a child/group"
                );
                ensure!(
                    if returned {
                        count(s, "effective_flags_or")? & 32 != 0 && s["cleanup_skipped"] == true
                    } else {
                        count(s, "effective_flags_or")? & 32 == 0
                    },
                    "cleanup skip gate differs"
                );
            } else {
                ensure!(
                    groups > 0
                        && s["pending_group"] == groups - 1
                        && bounded(&s["pending_slot"], 3)
                        && bounded(&s["child_reference"], u64::from(u32::MAX))
                        && s["factory_class"] == ((count(s, "child_reference")? >> 24) & 0x7f)
                        && bounded(&s["child_flags"], 65535)
                        && bounded(&s["child_divisor"], u64::from(u32::MAX))
                        && count(s, "effective_flags_or")? & count(s, "child_flags")?
                            == count(s, "child_flags")?,
                    "factory request differs"
                );
                if s["stage"] == "child_factory_pending" {
                    ensure!(
                        s["factory_class"] != 4
                            && created == completed
                            && s["child_header"].is_null(),
                        "class04 allocation skipped"
                    );
                } else {
                    ensure!(
                        s["factory_class"] == 4
                            && created
                                == completed
                                    + u64::from(s["stage"] != "child_continuation_pending")
                            && s["child_header"]["slot"] == s["pending_slot"],
                        "current child allocation differs"
                    );
                    player_child::validate(&s["child_header"], &s["stage"])?;
                    if s["stage"] == "child_wrapper_pending" {
                        ensure!(count(s, "child_flags")? & 8 != 0, "wrapper gate differs");
                    }
                    let phase = &s["child_header"]["phase_zero"];
                    if let Some(bits) = phase["position"]["normalized_position_bits"].as_u64() {
                        let root = count(s, "child_flags")? & 2 != 0 && bits & 0x7fff_ffff != 0;
                        ensure!(
                            (phase["stage"] == "root_motion_pending") == root,
                            "position root-motion gate differs"
                        );
                    }
                }
            }
        }
        Some("mask_control_pending" | "mask_clear_pending") => ensure!(
            op == 0x0c00_0005
                && created == 0
                && s["child_header"].is_null()
                && no_set
                && no_child
                && groups == 0
                && count(s, "effective_flags_or")? == 0
                && bounded(&s["mask"], 65535)
                && (s["mask"] == 0) == (s["stage"] == "mask_clear_pending"),
            "mask boundary differs"
        ),
        Some("opcode_pending") => ensure!(
            !matches!(op, 0x0c00_0002 | 0x0c00_0003 | 0x0c00_0005)
                && created == 0
                && s["child_header"].is_null()
                && no_set
                && no_child
                && groups == 0
                && count(s, "effective_flags_or")? == 0
                && s["mask"].is_null(),
            "opcode boundary differs"
        ),
        _ => anyhow::bail!("unsupported command boundary"),
    }
    Ok(())
}
#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;
    pub(in crate::menu::game_session) mod returned;
    pub(in super::super) fn fixture() -> Value {
        json!({"stage":"child_initializer_pending","opcode":0x0c00_0002,"set_reference":0x0a00_00ab,"set_index":0,"set_groups":1,
            "groups_visited":1,"weight_writes":1,"pending_group":0,"pending_slot":1,"child_reference":0x0400_00ab,"factory_class":4,
            "child_flags":32,"child_divisor":3,"mask":null,"mask_effects":null,"children_created":1,"children_completed":0,"children_inserted":0,"head_copies":0,"active_slots":1,"effective_flags_or":32,"last_completed_child":null,"cleanup_skipped":false,"child_header":player_child::tests::fixture(),"command_returned":false,"cleanup_complete":false,
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
        state["effective_flags_or"] = json!(0);
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
