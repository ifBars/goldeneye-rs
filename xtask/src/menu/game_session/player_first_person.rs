use super::{closed, count, player_refresh, player_skeleton};
use anyhow::{Result, ensure};
use serde_json::Value;

pub(super) fn validate(state: &Value) -> Result<()> {
    closed(
        state,
        &[
            "helper_returned",
            "identity_registered",
            "item_list_ordinal",
            "ordered_links_bound",
            "masks_configured",
            "table_shared",
            "pose_target_assigned",
            "visual_postbind_skipped",
            "skeleton",
            "bind_refresh",
            "state_sha256",
        ],
    )?;
    ensure!(
        [
            "helper_returned",
            "identity_registered",
            "ordered_links_bound",
            "masks_configured",
            "table_shared",
            "pose_target_assigned",
            "visual_postbind_skipped"
        ]
        .into_iter()
        .all(|key| state[key] == true)
            && count(state, "item_list_ordinal")? == 1
            && state["state_sha256"]
                .as_str()
                .is_some_and(|hash| hash.len() == 64
                    && hash
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))),
        "FP helper evidence is contradictory"
    );
    player_skeleton::validate(&state["skeleton"])?;
    player_refresh::validate(&state["bind_refresh"])?;
    Ok(())
}
#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;
    pub(in super::super) fn fixture() -> Value {
        let mut refresh = player_refresh::tests::fixture();
        refresh["refresh_ordinal"] = json!(2);
        refresh["allocation_ordinal"] = json!(2);
        json!({"helper_returned":true,"identity_registered":true,"item_list_ordinal":1,"ordered_links_bound":true,
            "masks_configured":true,"table_shared":true,"pose_target_assigned":true,"visual_postbind_skipped":true,
            "skeleton":player_skeleton::tests::fixture(),"bind_refresh":refresh,"state_sha256":"f".repeat(64)})
    }
    #[test]
    fn rejects_detached_or_partial_fp_and_pose_claims() {
        validate(&fixture()).unwrap();
        for (key, value) in [
            ("helper_returned", json!(false)),
            ("identity_registered", json!(false)),
            ("item_list_ordinal", json!(0)),
            ("ordered_links_bound", json!(false)),
            ("masks_configured", json!(false)),
            ("table_shared", json!(false)),
            ("pose_target_assigned", json!(false)),
            ("visual_postbind_skipped", json!(false)),
            ("skeleton", Value::Null),
            ("bind_refresh", Value::Null),
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
