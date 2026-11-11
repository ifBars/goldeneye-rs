use super::{closed, count};
use anyhow::{Result, ensure};
use serde_json::Value;

pub(super) fn validate(state: &Value) -> Result<()> {
    closed(
        state,
        &[
            "identity_registered",
            "item_backlink_bound",
            "item_list_member",
            "item_list_ordinal",
            "item_components",
            "registry_identities",
            "component_flag_8e",
            "head_link_null",
            "next_link_bound",
            "state_sha256",
        ],
    )?;
    ensure!(
        [
            "identity_registered",
            "item_backlink_bound",
            "item_list_member",
            "head_link_null",
            "next_link_bound"
        ]
        .into_iter()
        .all(|key| state[key] == true)
            && state["item_list_ordinal"] == 0
            && state["item_components"] == 2
            && (2..=65_536).contains(&count(state, "registry_identities")?)
            && state["component_flag_8e"] == 0
            && state["state_sha256"]
                .as_str()
                .is_some_and(|hash| hash.len() == 64
                    && hash
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))),
        "TP attachment evidence is contradictory"
    );
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;
    pub(in super::super) fn fixture() -> Value {
        json!({"identity_registered":true,"item_backlink_bound":true,"item_list_member":true,
            "item_list_ordinal":0,"item_components":2,"registry_identities":2,"component_flag_8e":0,
            "head_link_null":true,"next_link_bound":true,"state_sha256":"c".repeat(64)})
    }
    #[test]
    fn rejects_missing_owner_membership_activation_and_private_payload_shortcuts() {
        validate(&fixture()).unwrap();
        for (key, value) in [
            ("identity_registered", json!(false)),
            ("item_backlink_bound", json!(false)),
            ("item_list_member", json!(false)),
            ("head_link_null", json!(false)),
            ("next_link_bound", json!(false)),
            ("item_list_ordinal", json!(1)),
            ("item_components", json!(0)),
            ("registry_identities", json!(0)),
            ("component_flag_8e", json!(1)),
            ("state_sha256", json!("short")),
            ("source_pointer", json!(123)),
        ] {
            let mut state = fixture();
            state[key] = value;
            assert!(validate(&state).is_err(), "accepted {key}");
        }
    }
}
