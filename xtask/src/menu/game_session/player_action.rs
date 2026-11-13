use super::{
    closed, count, player_capture, player_command, player_dispatch, player_fp, player_selector,
};
use anyhow::{Result, ensure};
use serde_json::Value;

pub(super) fn validate(state: &Value) -> Result<()> {
    closed(
        state,
        &[
            "stage",
            "generic_request",
            "parameter_bits",
            "direction_raw",
            "weapon_key",
            "variant",
            "remap_calls",
            "map_rows_read",
            "fp_mapped",
            "tp_mapped",
            "tp_secondary_mapped",
            "source_tables_reused",
            "mode_controls_created",
            "mode_history_updated",
            "callback_returned",
            "state_sha256",
            "mapping_state_sha256",
            "tp_capture",
            "tp_selector",
            "tp_command",
            "tp_dispatch",
            "fp_dispatch",
        ],
    )?;
    ensure!(
        player_selector::dependency(&state["stage"]).is_some()
            && state["generic_request"] == 0x0900_071eu32
            && state["parameter_bits"] == 0
            && state["direction_raw"] == 255
            && count(state, "weapon_key")? <= u64::from(u32::MAX)
            && count(state, "variant")? <= 1
            && state["remap_calls"] == 3
            && count(state, "map_rows_read")? <= 3 * 4096
            && ["fp_mapped", "tp_mapped", "tp_secondary_mapped"]
                .into_iter()
                .all(|key| state[key]
                    .as_u64()
                    .is_some_and(|value| value > 0 && value <= u64::from(u32::MAX)))
            && state["source_tables_reused"] == true
            && state["mode_controls_created"] == 4
            && state["mode_history_updated"].is_boolean()
            && state["callback_returned"] == false
            && ["state_sha256", "mapping_state_sha256"]
                .into_iter()
                .all(
                    |key| state[key].as_str().is_some_and(|hash| hash.len() == 64
                        && hash
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
                ),
        "initial action evidence is contradictory"
    );
    player_capture::validate(&state["tp_capture"])?;
    player_selector::validate(&state["tp_selector"], state)?;
    player_command::validate(&state["tp_command"], state)?;
    player_dispatch::validate(&state["tp_dispatch"], state)?;
    player_fp::validate(&state["fp_dispatch"], state)?;
    ensure!(
        state["mode_history_updated"]
            == (state["tp_dispatch"]["history_updated"] == true
                || state["fp_dispatch"]["history_updated"] == true),
        "initial action history aggregate differs"
    );
    Ok(())
}
#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;
    pub(in super::super) fn fixture() -> Value {
        json!({"stage":"tp_command_pending","generic_request":0x0900_071e,"parameter_bits":0,"direction_raw":255,
            "weapon_key":0x0200_00ab,"variant":0,"remap_calls":3,"map_rows_read":20,"fp_mapped":0x0900_00ab,
            "tp_mapped":0x0900_00ac,"tp_secondary_mapped":0x0900_00ad,"source_tables_reused":true,
            "mode_controls_created":4,"mode_history_updated":false,"callback_returned":false,
            "state_sha256":"a".repeat(64),"mapping_state_sha256":"b".repeat(64),"tp_capture":player_capture::tests::fixture(),"tp_selector":player_selector::tests::fixture(),"tp_command":player_command::tests::fixture(),"tp_dispatch":player_dispatch::tests::fixture(),"fp_dispatch":null})
    }
    #[test]
    fn rejects_partial_mapping_and_early_callback_or_mode_completion() {
        validate(&fixture()).unwrap();
        for (key, value) in [
            ("stage", json!("complete")),
            ("generic_request", json!(0)),
            ("parameter_bits", json!(1)),
            ("direction_raw", json!(1)),
            ("weapon_key", json!(-1)),
            ("variant", json!(2)),
            ("remap_calls", json!(2)),
            ("map_rows_read", json!(12289)),
            ("fp_mapped", json!(0)),
            ("tp_mapped", json!(0)),
            ("tp_secondary_mapped", json!(0)),
            ("source_tables_reused", json!(false)),
            ("mode_controls_created", json!(0)),
            ("mode_history_updated", json!(true)),
            ("callback_returned", json!(true)),
            ("state_sha256", json!("short")),
            ("mapping_state_sha256", json!("short")),
            ("source_table", json!([1, 2, 3])),
        ] {
            let mut state = fixture();
            state[key] = value;
            assert!(validate(&state).is_err(), "accepted {key}");
        }
        for key in fixture().as_object().unwrap().keys() {
            let mut state = fixture();
            state.as_object_mut().unwrap().remove(key);
            assert!(validate(&state).is_err(), "accepted missing {key}");
        }
    }
}
