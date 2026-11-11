use super::{closed, count};
use anyhow::{Result, ensure};
use serde_json::Value;
pub(super) fn validate(state: &Value, expected_textures: u64) -> Result<()> {
    closed(
        state,
        &[
            "wrapper_reused",
            "secondary_skin_bound",
            "skin_requests",
            "texture_requests",
            "texture_targets_resident",
            "override_generation",
            "override_capacity",
            "override_growth",
            "override_count",
            "item_collection_bound",
            "configured_headers",
            "callback_bindings_complete",
            "state_sha256",
        ],
    )?;
    ensure!(
        [
            "wrapper_reused",
            "secondary_skin_bound",
            "item_collection_bound"
        ]
        .into_iter()
        .all(|key| state[key] == true)
            && state["skin_requests"] == 1
            && count(state, "texture_requests")? == expected_textures
            && count(state, "texture_targets_resident")? == expected_textures
            && expected_textures <= 4096
            && state["override_generation"] == 1
            && state["override_capacity"] == 0
            && state["override_growth"] == 16
            && state["override_count"] == 0
            && state["configured_headers"] == 4
            && state["callback_bindings_complete"] == false
            && state["state_sha256"]
                .as_str()
                .is_some_and(|hash| hash.len() == 64
                    && hash
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))),
        "TP visual postbind evidence is contradictory"
    );
    Ok(())
}
#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;
    pub(in super::super) fn fixture() -> Value {
        json!({"wrapper_reused":true,"secondary_skin_bound":true,"skin_requests":1,"texture_requests":3,"texture_targets_resident":3,
            "override_generation":1,"override_capacity":0,"override_growth":16,"override_count":0,"item_collection_bound":true,
            "configured_headers":4,"callback_bindings_complete":false,"state_sha256":"d".repeat(64)})
    }
    #[test]
    fn rejects_missing_bindings_partial_requests_or_early_callbacks() {
        validate(&fixture(), 3).unwrap();
        for (key, value) in [
            ("wrapper_reused", json!(false)),
            ("secondary_skin_bound", json!(false)),
            ("skin_requests", json!(0)),
            ("texture_requests", json!(2)),
            ("texture_targets_resident", json!(2)),
            ("override_generation", json!(0)),
            ("override_capacity", json!(1)),
            ("override_growth", json!(0)),
            ("override_count", json!(1)),
            ("item_collection_bound", json!(false)),
            ("configured_headers", json!(1)),
            ("callback_bindings_complete", json!(true)),
            ("state_sha256", json!("short")),
            ("raw_bones", json!([1, 2, 3])),
        ] {
            let mut state = fixture();
            state[key] = value;
            assert!(validate(&state, 3).is_err(), "accepted {key}");
        }
    }
}
