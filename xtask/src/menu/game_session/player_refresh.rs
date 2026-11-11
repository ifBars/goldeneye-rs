use super::{closed, count};
use anyhow::{Result, ensure};
use serde_json::Value;

pub(super) fn validate(state: &Value) -> Result<()> {
    closed(
        state,
        &[
            "arena_bytes",
            "reserved_bytes",
            "reservation_start",
            "cursor_after",
            "live_reservations_after",
            "refresh_ordinal",
            "allocation_ordinal",
            "regions_bound",
            "flags_after",
            "scope_released",
            "state_sha256",
        ],
    )?;
    let start = count(state, "reservation_start")?;
    let allocation = count(state, "allocation_ordinal")?;
    let refresh = count(state, "refresh_ordinal")?;
    ensure!(
        state["arena_bytes"] == 0x4000
            && state["reserved_bytes"] == 0x2200
            && start < 0x1e00
            && start.is_multiple_of(16)
            && count(state, "cursor_after")? == start
            && refresh > 0
            && allocation >= refresh
            && count(state, "live_reservations_after")? < allocation
            && state["regions_bound"] == 4
            && state["flags_after"] == 0
            && state["scope_released"] == true
            && state["state_sha256"]
                .as_str()
                .is_some_and(|hash| hash.len() == 64
                    && hash
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))),
        "TP bind refresh evidence is contradictory"
    );
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;
    pub(in super::super) fn fixture() -> Value {
        json!({"arena_bytes":0x4000,"reserved_bytes":0x2200,"reservation_start":0,"cursor_after":0,
            "live_reservations_after":0,"refresh_ordinal":1,"allocation_ordinal":1,"regions_bound":4,
            "flags_after":0,"scope_released":true,"state_sha256":"b".repeat(64)})
    }
    #[test]
    fn accepts_nested_prefix_and_rejects_fake_release_context_or_capacity() {
        validate(&fixture()).unwrap();
        let mut nested = fixture();
        nested["reservation_start"] = json!(32);
        nested["cursor_after"] = json!(32);
        nested["live_reservations_after"] = json!(1);
        nested["allocation_ordinal"] = json!(2);
        validate(&nested).unwrap();
        for (key, value) in [
            ("arena_bytes", json!(0)),
            ("reserved_bytes", json!(0)),
            ("reservation_start", json!(0x1e00)),
            ("cursor_after", json!(16)),
            ("live_reservations_after", json!(1)),
            ("refresh_ordinal", json!(0)),
            ("allocation_ordinal", json!(0)),
            ("regions_bound", json!(3)),
            ("flags_after", json!(0x4000)),
            ("scope_released", json!(false)),
            ("state_sha256", json!("short")),
            ("payload", json!([1, 2, 3])),
        ] {
            let mut state = fixture();
            state[key] = value;
            assert!(validate(&state).is_err(), "accepted {key}");
        }
        nested["reservation_start"] = json!(17);
        nested["cursor_after"] = json!(17);
        assert!(validate(&nested).is_err());
    }
}
