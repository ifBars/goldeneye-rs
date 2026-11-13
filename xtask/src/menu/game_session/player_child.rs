use super::closed;
use anyhow::{Result, ensure};
use serde_json::Value;
mod phase_zero;
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
            "phase_zero",
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
            && state["phase_zero_initialized"].is_boolean()
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
    let phase = &state["phase_zero"];
    if !phase.is_null() {
        phase_zero::validate(phase)?;
    }
    let no_play = phase["stage"] == "returned_without_play";
    ensure!(
        state["phase_zero_initialized"] == no_play,
        "phase-zero return differs"
    );
    ensure!(
        if state["body_bound"] == true {
            state["metadata_cached"] == true
                && stage
                    == if no_play {
                        "child_effects_pending"
                    } else {
                        "child_initializer_pending"
                    }
                && state["initializer_returned"] == no_play
                && state["factory_returned"] == no_play
        } else {
            phase.is_null()
                && stage == "child_effects_pending"
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
        "generation_observed":0,"cache_generation":0,"metadata_cached":true,"descriptor_index":2,"body_bound":true,"phase_zero":phase_zero::fixture(),
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
        state["phase_zero"] = Value::Null;
        state["initializer_returned"] = json!(true);
        state["factory_returned"] = json!(true);
        validate(&state, &json!("child_effects_pending")).unwrap();
        state["metadata_cached"] = json!(false);
        state["descriptor_index"] = Value::Null;
        state["cache_generation"] = json!(u32::MAX);
        validate(&state, &json!("child_effects_pending")).unwrap();
    }
    #[test]
    fn phase_zero_receipt_rejects_unreached_effects_and_malformed_counts() {
        let base = phase_zero::fixture();
        for (key, value) in [
            ("stage", json!("complete")),
            ("delta_bits", json!(0x7f800000)),
            ("duration_raw", json!(-1)),
            ("frame_bits", json!(0x7fc00000)),
            ("entries_visited", json!(0)),
            ("selected_offset", Value::Null),
            ("normal_requested", json!(256)),
            ("secondary_requested", Value::Null),
            ("normal_bound", json!(2)),
            ("secondary_bound", json!(1)),
            ("clip_records", json!(0)),
            ("heap_allocated", json!(true)),
            ("initial_index_raw", json!(256)),
            ("initial_index_clamped", json!(1)),
            ("frame_range", json!(1)),
            ("setup_returned", json!(true)),
            ("flags_applied", json!(true)),
            ("pending_file", json!(7)),
            ("payload", json!([1])),
        ] {
            let mut v = base.clone();
            v[key] = value;
            assert!(phase_zero::validate(&v).is_err(), "accepted {key}");
        }
        for key in base.as_object().unwrap().keys() {
            let mut v = base.clone();
            v.as_object_mut().unwrap().remove(key);
            assert!(phase_zero::validate(&v).is_err(), "accepted absent {key}");
        }
        let mut child = fixture();
        child["phase_zero"]["stage"] = json!("returned_without_play");
        for key in [
            "selected_offset",
            "normal_requested",
            "secondary_requested",
            "initial_index_raw",
            "initial_index_clamped",
            "frame_range",
        ] {
            child["phase_zero"][key] = Value::Null;
        }
        for key in ["normal_bound", "clip_records"] {
            child["phase_zero"][key] = json!(0);
        }
        for key in [
            "phase_zero_initialized",
            "initializer_returned",
            "factory_returned",
        ] {
            child[key] = json!(true);
        }
        validate(&child, &json!("child_effects_pending")).unwrap();
        assert!(validate(&child, &json!("child_initializer_pending")).is_err());
    }
}
