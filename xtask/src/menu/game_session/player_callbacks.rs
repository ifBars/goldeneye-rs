use super::closed;
use anyhow::{Result, ensure};
use serde_json::Value;

pub(super) fn validate(state: &Value) -> Result<()> {
    closed(
        state,
        &[
            "interface_bound",
            "table_shared",
            "callback_slots",
            "noop_slots",
            "pose_target_assigned",
            "pose_postprocess_implemented",
            "tp_helper_returned",
            "events_executed",
            "state_sha256",
        ],
    )?;
    ensure!(
        [
            "interface_bound",
            "table_shared",
            "pose_target_assigned",
            "tp_helper_returned"
        ]
        .into_iter()
        .all(|key| state[key] == true)
            && state["callback_slots"] == 4
            && state["noop_slots"] == 2
            && state["pose_postprocess_implemented"] == false
            && state["events_executed"] == false
            && state["state_sha256"]
                .as_str()
                .is_some_and(|hash| hash.len() == 64
                    && hash
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))),
        "TP callback evidence is contradictory"
    );
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;
    pub(in super::super) fn fixture() -> Value {
        json!({"interface_bound":true,"table_shared":true,"callback_slots":4,"noop_slots":2,
            "pose_target_assigned":true,"pose_postprocess_implemented":false,"tp_helper_returned":true,
            "events_executed":false,"state_sha256":"e".repeat(64)})
    }
    #[test]
    fn rejects_unbound_routes_and_unexecuted_work_claimed_as_complete() {
        validate(&fixture()).unwrap();
        for (key, value) in [
            ("interface_bound", json!(false)),
            ("table_shared", json!(false)),
            ("callback_slots", json!(3)),
            ("noop_slots", json!(0)),
            ("pose_target_assigned", json!(false)),
            ("pose_postprocess_implemented", json!(true)),
            ("tp_helper_returned", json!(false)),
            ("events_executed", json!(true)),
            ("state_sha256", json!("short")),
            ("raw_code", json!([1, 2, 3])),
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
