use super::{closed, count, player_selector};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
mod cache;
fn hash(v: &Value) -> bool {
    v.as_str().is_some_and(|h| {
        h.len() == 64
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
pub(super) fn validate(p: &Value, c: &Value) -> Result<()> {
    if p.is_null() {
        ensure!(
            !player_selector::at_least(&c["stage"], 6),
            "cache/preload boundary precedes actual initializer prefix"
        );
        return Ok(());
    }
    closed(
        p,
        &[
            "stage",
            "pending_dependency",
            "item_flags_before",
            "item_flags_after",
            "mode_written",
            "dirty_cleared",
            "scalars_written",
            "receiver_searches",
            "receivers_found",
            "cache_enabled",
            "invalidated",
            "caches",
            "state_sha256",
        ],
    )?;
    let stage = [
        "item_flags",
        "item_mode",
        "item_dirty",
        "base_scalars",
        "first_person",
        "third_person",
        "caches",
        "cache_flag",
        "invalidate",
        "awaiting_preload",
        "awaiting_tail",
    ]
    .iter()
    .position(|s| p["stage"] == *s)
    .context("unsupported initializer-prefix stage")?;
    let found = p["receivers_found"]
        .as_array()
        .filter(|v| v.len() == 2 && v.iter().all(Value::is_boolean))
        .context("initializer FP/TP receiver results absent")?;
    ensure!(
        c["component_tail"]["helper_returned"] == true
            && c["component_tail"]["helper_return"] == 1
            && p["mode_written"] == (stage >= 2)
            && p["dirty_cleared"] == (stage >= 3)
            && p["scalars_written"] == (stage >= 4)
            && count(p, "receiver_searches")?
                == if stage < 5 {
                    0
                } else if stage == 5 {
                    1
                } else {
                    2
                }
            && p["invalidated"].is_boolean()
            && hash(&p["state_sha256"]),
        "initializer stores or caller scope skip source ordering"
    );
    if stage == 0 {
        ensure!(
            p["item_flags_before"].is_null() && p["item_flags_after"].is_null(),
            "initializer U16 flags store ran early"
        );
    } else {
        ensure!(
            count(p, "item_flags_before")? <= 65535
                && count(p, "item_flags_after")? == (count(p, "item_flags_before")? | 0xc),
            "initializer actual U16 flags OR differs"
        );
    }
    if stage < 5 {
        ensure!(
            found.iter().all(|v| *v == false),
            "initializer receiver found before lookup"
        );
    }
    if stage == 5 {
        ensure!(
            found[1] == false,
            "initializer TP search precedes FP search"
        );
    }
    let both = found.iter().all(|v| *v == true);
    let root = match stage {
        6 => "player_transform_caches_pending",
        9 => "player_animation_preload_pending",
        10 => "player_initializer_tail_pending",
        _ => "player_initializer_pending",
    };
    let dependency = match stage {
        6 => "player_transform_caches",
        9 => "player_animation_preload",
        10 => "player_initializer_tail",
        _ => "player_initializer_prefix",
    };
    ensure!(
        c["stage"] == root && p["pending_dependency"] == dependency,
        "initializer root/dependency differs from its actual reached stage"
    );
    if !p["caches"].is_null() {
        ensure!(
            stage >= 6 && both,
            "initializer cache operation bypassed actual receiver gates"
        );
        cache::validate(&p["caches"], p)?;
    }
    if (7..=9).contains(&stage) {
        ensure!(
            both && !p["caches"].is_null() && p["caches"]["trace"]["returned"] == true,
            "initializer advanced before real cache call return"
        );
    }
    if stage < 8 {
        ensure!(
            p["cache_enabled"].is_null(),
            "initializer source cache flag read before cache return"
        );
    }
    if stage != 10 {
        ensure!(
            p["invalidated"] == false,
            "initializer invalidation ran before its source arm"
        );
    }
    match stage {
        8 => ensure!(
            both && p["cache_enabled"]
                .as_u64()
                .is_some_and(|v| v <= 255 && v != 1),
            "initializer invalidation skipped its actual cache flag"
        ),
        9 => ensure!(
            p["cache_enabled"] == 1 && p["invalidated"] == false,
            "animation preload must remain pending with source flag1 unchanged"
        ),
        10 => {
            if both {
                ensure!(
                    p["cache_enabled"]
                        .as_u64()
                        .is_some_and(|v| v <= 255 && v != 1)
                        && p["invalidated"] == true
                        && !p["caches"].is_null()
                        && p["caches"]["trace"]["returned"] == true,
                    "initializer flag0 arm did not actually invalidate its selection"
                );
            } else {
                ensure!(
                    p["cache_enabled"].is_null()
                        && p["invalidated"] == false
                        && p["caches"].is_null(),
                    "missing receiver branch invented a cache call, flag read or invalidation"
                );
            }
        }
        _ => {}
    }
    Ok(())
}
#[cfg(test)]
pub(super) mod tests;
