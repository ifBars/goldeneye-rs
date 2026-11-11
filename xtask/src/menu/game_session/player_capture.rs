use super::{closed, count, player_refresh};
use anyhow::{Result, ensure};
use serde_json::Value;
pub(super) fn validate(state: &Value) -> Result<()> {
    closed(
        state,
        &[
            "physical_slots",
            "controls",
            "bones_per_control",
            "auxiliary_words_per_control",
            "native_header_bytes",
            "native_backing_bytes_per_control",
            "active_slots",
            "capture_mask",
            "preserve_mask",
            "update_flags_after",
            "frame_stamp_known",
            "postprocess_executed",
            "fallback",
            "fallback_writes",
            "refresh",
            "state_sha256",
        ],
    )?;
    let bones = count(state, "bones_per_control")?;
    let auxiliary = count(state, "auxiliary_words_per_control")?;
    ensure!(
        state["physical_slots"] == 4
            && state["controls"] == 4
            && bones <= 128
            && auxiliary <= 4096
            && state["native_header_bytes"] == 0x54
            && count(state, "native_backing_bytes_per_control")? == bones * 32 + auxiliary * 4
            && state["active_slots"] == 1
            && state["capture_mask"] == 15
            && state["preserve_mask"] == 3
            && state["update_flags_after"] == 3
            && state["frame_stamp_known"] == false
            && state["postprocess_executed"] == false
            && state["fallback"] == 0x0900_06c0u32
            && count(state, "fallback_writes")? >= 2
            && state["state_sha256"]
                .as_str()
                .is_some_and(|hash| hash.len() == 64
                    && hash
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))),
        "TP control capture evidence is contradictory"
    );
    player_refresh::validate(&state["refresh"])?;
    Ok(())
}
#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;
    pub(in super::super) fn fixture() -> Value {
        let mut refresh = player_refresh::tests::fixture();
        refresh["refresh_ordinal"] = json!(3);
        refresh["allocation_ordinal"] = json!(3);
        json!({"physical_slots":4,"controls":4,"bones_per_control":2,"auxiliary_words_per_control":4,"native_header_bytes":0x54,
            "native_backing_bytes_per_control":80,"active_slots":1,"capture_mask":15,"preserve_mask":3,"update_flags_after":3,
            "frame_stamp_known":false,"postprocess_executed":false,"fallback":0x0900_06c0,"fallback_writes":2,"refresh":refresh,"state_sha256":"c".repeat(64)})
    }
    #[test]
    fn rejects_partial_lists_bad_backing_or_pose_execution_claims() {
        validate(&fixture()).unwrap();
        for (key, value) in [
            ("physical_slots", json!(1)),
            ("controls", json!(2)),
            ("bones_per_control", json!(129)),
            ("auxiliary_words_per_control", json!(4097)),
            ("native_header_bytes", json!(0)),
            ("native_backing_bytes_per_control", json!(79)),
            ("active_slots", json!(4)),
            ("capture_mask", json!(3)),
            ("preserve_mask", json!(15)),
            ("update_flags_after", json!(11)),
            ("frame_stamp_known", json!(true)),
            ("postprocess_executed", json!(true)),
            ("fallback", json!(0)),
            ("fallback_writes", json!(1)),
            ("refresh", Value::Null),
            ("state_sha256", json!("short")),
            ("pose", json!([1, 2, 3])),
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
