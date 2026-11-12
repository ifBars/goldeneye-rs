use super::closed;
use anyhow::{Result, ensure};
use serde_json::Value;
pub(super) fn validate(state: &Value, stage: &Value) -> Result<()> {
    closed(
        state,
        &[
            "native_header_bytes",
            "resource_reused",
            "context_bound",
            "generation_observed",
            "cache_generation",
            "metadata_cached",
            "descriptor_index",
            "body_bound",
            "phase_zero_initialized",
            "initializer_returned",
            "factory_returned",
            "layer_inserted",
            "state_sha256",
        ],
    )?;
    let word = |key: &str| {
        state[key]
            .as_u64()
            .is_some_and(|value| value <= u64::from(u32::MAX))
    };
    ensure!(
        state["native_header_bytes"] == 0x90
            && state["resource_reused"] == true
            && state["context_bound"] == true
            && word("generation_observed")
            && word("cache_generation")
            && state["metadata_cached"].is_boolean()
            && state["body_bound"].is_boolean()
            && state["phase_zero_initialized"] == false
            && state["layer_inserted"] == false
            && state["state_sha256"]
                .as_str()
                .is_some_and(|hash| hash.len() == 64
                    && hash
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
            && if state["metadata_cached"] == true {
                state["descriptor_index"]
                    .as_u64()
                    .is_some_and(|index| index < 32767)
                    && state["cache_generation"] == state["generation_observed"]
            } else {
                state["descriptor_index"].is_null() && state["cache_generation"] == u32::MAX
            },
        "TP child header/cache evidence differs"
    );
    ensure!(
        if state["body_bound"] == true {
            state["metadata_cached"] == true
                && stage == "child_initializer_pending"
                && state["initializer_returned"] == false
                && state["factory_returned"] == false
        } else {
            stage == "child_effects_pending"
                && state["initializer_returned"] == true
                && state["factory_returned"] == true
        },
        "TP child initializer return or pending phase differs"
    );
    Ok(())
}
#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;
    pub(in super::super) fn fixture() -> Value {
        json!({"native_header_bytes":0x90,"resource_reused":true,"context_bound":true,
        "generation_observed":0,"cache_generation":0,"metadata_cached":true,"descriptor_index":2,"body_bound":true,
        "phase_zero_initialized":false,"initializer_returned":false,"factory_returned":false,"layer_inserted":false,"state_sha256":"d".repeat(64)})
    }
    #[test]
    fn header_cannot_claim_later_initializer_membership_or_missing_cache_data() {
        validate(&fixture(), &json!("child_initializer_pending")).unwrap();
        for (key, value) in [
            ("native_header_bytes", json!(0x80)),
            ("resource_reused", json!(false)),
            ("context_bound", json!(false)),
            ("generation_observed", json!(-1)),
            ("cache_generation", json!(7)),
            ("metadata_cached", json!(false)),
            ("descriptor_index", Value::Null),
            ("body_bound", json!(false)),
            ("phase_zero_initialized", json!(true)),
            ("initializer_returned", json!(true)),
            ("factory_returned", json!(true)),
            ("layer_inserted", json!(true)),
            ("state_sha256", json!("short")),
            ("source", json!([1, 2, 3])),
        ] {
            let mut state = fixture();
            state[key] = value;
            assert!(
                validate(&state, &json!("child_initializer_pending")).is_err(),
                "accepted {key}"
            );
        }
        for key in fixture().as_object().unwrap().keys() {
            let mut state = fixture();
            state.as_object_mut().unwrap().remove(key);
            assert!(validate(&state, &json!("child_initializer_pending")).is_err());
        }
        let mut state = fixture();
        state["body_bound"] = json!(false);
        state["initializer_returned"] = json!(true);
        state["factory_returned"] = json!(true);
        validate(&state, &json!("child_effects_pending")).unwrap();
        state["metadata_cached"] = json!(false);
        state["descriptor_index"] = Value::Null;
        state["cache_generation"] = json!(u32::MAX);
        validate(&state, &json!("child_effects_pending")).unwrap();
    }
}
