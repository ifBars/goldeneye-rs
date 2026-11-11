use super::{closed, count};
use anyhow::{Result, ensure};
use serde_json::Value;

pub(super) fn validate(state: &Value) -> Result<()> {
    closed(
        state,
        &[
            "native_runtime_bytes",
            "native_storage_bytes",
            "bones",
            "roots",
            "auxiliary_words",
            "primary_bound",
            "secondary_bound",
            "animator_backlink_bound",
            "component_resource_bound",
            "frame_stamp_known",
            "empty_refresh_complete",
            "retained_skeleton_file_bytes",
            "retained_component_file_bytes",
            "state_sha256",
        ],
    )?;
    let bones = count(state, "bones")?;
    let auxiliary = count(state, "auxiliary_words")?;
    ensure!(
        state["native_runtime_bytes"] == 0x164
            && bones <= 128
            && count(state, "roots")? <= bones
            && auxiliary <= 4096
            && count(state, "native_storage_bytes")? == bones * 0x88 + auxiliary * 4
            && [
                "primary_bound",
                "secondary_bound",
                "animator_backlink_bound",
                "component_resource_bound"
            ]
            .into_iter()
            .all(|key| state[key] == true)
            && state["frame_stamp_known"] == false
            && state["empty_refresh_complete"] == false
            && [
                "retained_skeleton_file_bytes",
                "retained_component_file_bytes"
            ]
            .into_iter()
            .all(|key| state[key]
                .as_u64()
                .is_some_and(|bytes| (0xc8..=64 * 1024 * 1024).contains(&bytes)))
            && state["state_sha256"]
                .as_str()
                .is_some_and(|hash| hash.len() == 64
                    && hash
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))),
        "TP skeleton owner evidence is contradictory"
    );
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;
    pub(in super::super) fn fixture() -> Value {
        json!({"native_runtime_bytes":0x164,"native_storage_bytes":288,"bones":2,"roots":1,"auxiliary_words":4,
            "primary_bound":true,"secondary_bound":true,"animator_backlink_bound":true,"component_resource_bound":true,
            "frame_stamp_known":false,"empty_refresh_complete":false,"retained_skeleton_file_bytes":1024,
            "retained_component_file_bytes":2048,"state_sha256":"a".repeat(64)})
    }
    #[test]
    fn rejects_missing_backlinks_fake_refresh_bad_counts_or_private_payloads() {
        validate(&fixture()).unwrap();
        for (key, value) in [
            ("primary_bound", json!(false)),
            ("secondary_bound", json!(false)),
            ("animator_backlink_bound", json!(false)),
            ("component_resource_bound", json!(false)),
            ("empty_refresh_complete", json!(true)),
            ("frame_stamp_known", json!(true)),
            ("bones", json!(129)),
            ("roots", json!(3)),
            ("auxiliary_words", json!(4097)),
            ("native_storage_bytes", json!(287)),
            ("native_runtime_bytes", json!(0)),
            ("retained_skeleton_file_bytes", json!(0)),
            ("retained_component_file_bytes", json!(0)),
            ("state_sha256", json!("short")),
            ("pose", json!([0, 0, 0, 1])),
        ] {
            let mut state = fixture();
            state[key] = value;
            assert!(validate(&state).is_err(), "accepted {key}");
        }
    }
}
