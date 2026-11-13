use super::{closed, count};
use anyhow::{Result, ensure};
use serde_json::Value;

fn word(value: &Value) -> bool {
    value
        .as_u64()
        .is_some_and(|word| word <= u64::from(u32::MAX))
}
pub(super) fn dependency(stage: &Value) -> Option<&'static str> {
    match stage.as_str()? {
        "tp_command_pending" => Some("tp_animation_command"),
        "tp_setter_tail_pending" => Some("tp_mode_setter_tail"),
        "tp_dispatch_pending" => Some("tp_animation_update"),
        "fp_dispatch_pending" => Some("fp_mode_dispatch"),
        "fp_command_pending" => Some("fp_animation_command"),
        "fp_setter_tail_pending" => Some("fp_mode_setter_tail"),
        "fp_dispatch_tail_pending" => Some("fp_dispatch_tail"),
        "initial_action_suffix_pending" => Some("initial_action_suffix"),
        "action_dispatch_repeat_pending" => Some("initial_action_repeat"),
        _ => None,
    }
}
pub(super) fn validate(state: &Value, action: &Value) -> Result<()> {
    let pending = action["stage"] == "tp_command_pending";
    ensure!(
        pending
            || matches!(
                action["stage"].as_str(),
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
        "TP selector parent stage differs"
    );
    validate_for(
        state,
        &action["tp_mapped"],
        &action["tp_capture"]["fallback"],
        pending,
    )
}
pub(super) fn validate_for(
    state: &Value,
    requested: &Value,
    fallback: &Value,
    pending: bool,
) -> Result<()> {
    closed(
        state,
        &[
            "stage",
            "requested",
            "previous",
            "fallback",
            "tracks_player_modes",
            "target_index",
            "attempts",
            "lookups",
            "transition_rows",
            "commands_issued",
            "commands_executed",
            "pending_ordinal",
            "pending_opcode",
            "return_word",
            "base_only",
            "command_effects_complete",
            "state_sha256",
        ],
    )?;
    let attempts = count(state, "attempts")?;
    let rows = count(state, "transition_rows")?;
    ensure!(
        word(&state["requested"])
            && state["requested"] == *requested
            && state["previous"] == 0
            && state["fallback"] == *fallback
            && state["tracks_player_modes"].is_boolean()
            && (state["target_index"].is_null() || word(&state["target_index"]))
            && attempts <= 2
            && count(state, "lookups")? == attempts + 1
            && rows <= 65536
            && count(state, "commands_executed")? <= 4096
            && count(state, "commands_issued")? <= 4096
            && state["base_only"] == true
            && state["command_effects_complete"].is_boolean()
            && state["state_sha256"]
                .as_str()
                .is_some_and(|hash| hash.len() == 64
                    && hash
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))),
        "TP selector input, residency or effect evidence differs"
    );
    match state["stage"].as_str() {
        Some("command_pending") => ensure!(
            pending
                && !state["target_index"].is_null()
                && attempts > 0
                && rows > 0
                && count(state, "commands_issued")? == count(state, "commands_executed")? + 1
                && state["pending_ordinal"]
                    .as_u64()
                    .is_some_and(|n| n <= count(state, "commands_executed").unwrap_or(0))
                && state["command_effects_complete"] == false
                && word(&state["pending_opcode"])
                && state["return_word"].is_null(),
            "TP pending command claims an execution or selector return"
        ),
        Some("returned") => ensure!(
            !pending
                && state["commands_issued"] == state["commands_executed"]
                && state["command_effects_complete"] == (count(state, "commands_executed")? > 0)
                && state["pending_ordinal"].is_null()
                && state["pending_opcode"].is_null()
                && word(&state["return_word"])
                && if state["target_index"].is_null() {
                    attempts == 0
                        && rows == 0
                        && state["return_word"] == 0
                        && state["commands_executed"] == 0
                } else {
                    attempts > 0
                },
            "TP returned selector still has a command or claims setter completion"
        ),
        _ => anyhow::bail!("TP selector boundary is unsupported"),
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;
    pub(in super::super) fn fixture() -> Value {
        json!({"stage":"command_pending","requested":0x0900_00ac,"previous":0,"fallback":0x0900_06c0,
            "tracks_player_modes":false,"target_index":1,"attempts":1,"lookups":2,"transition_rows":1,
            "commands_issued":1,"commands_executed":0,"pending_ordinal":0,"pending_opcode":0x0c00_0002,
            "return_word":null,"base_only":true,"command_effects_complete":false,"state_sha256":"c".repeat(64)})
    }
    #[test]
    fn rejects_early_effects_return_mismatches_missing_fields_and_payload() {
        let action = super::super::player_action::tests::fixture();
        validate(&fixture(), &action).unwrap();
        for (key, value) in [
            ("stage", json!("complete")),
            ("requested", json!(0)),
            ("previous", json!(1)),
            ("fallback", json!(0)),
            ("tracks_player_modes", Value::Null),
            ("target_index", Value::Null),
            ("attempts", json!(0)),
            ("lookups", json!(3)),
            ("transition_rows", json!(0)),
            ("commands_issued", json!(0)),
            ("commands_executed", json!(1)),
            ("pending_ordinal", json!(1)),
            ("pending_opcode", Value::Null),
            ("return_word", json!(0)),
            ("base_only", json!(false)),
            ("command_effects_complete", json!(true)),
            ("state_sha256", json!("short")),
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
        // Unknown opcodes are retained before interpretation, not executed.
        let mut state = fixture();
        state["pending_opcode"] = json!(0x0c00_00ffu32);
        validate(&state, &action).unwrap();
    }
    #[test]
    fn accepts_empty_return_and_missing_target_only_at_setter_tail() {
        let mut action = super::super::player_action::tests::fixture();
        action["stage"] = json!("tp_setter_tail_pending");
        let mut state = fixture();
        state["stage"] = json!("returned");
        state["commands_issued"] = json!(0);
        state["pending_ordinal"] = Value::Null;
        state["pending_opcode"] = Value::Null;
        state["return_word"] = json!(0x0900_00b0);
        validate(&state, &action).unwrap();
        state["target_index"] = Value::Null;
        state["attempts"] = json!(0);
        state["lookups"] = json!(1);
        state["transition_rows"] = json!(0);
        assert!(validate(&state, &action).is_err());
        state["return_word"] = json!(0);
        validate(&state, &action).unwrap();
        action["stage"] = json!("tp_command_pending");
        assert!(validate(&state, &action).is_err());
    }
}
