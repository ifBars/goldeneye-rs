//! Loading and development-checkpoint composition for the owned game session.
//! Progress follows actual controller stages, with no simulated percentage.
use super::{SessionStage, Snapshot};
use crate::{
    frontend::{Assets, BLUE, WHITE},
    menu::view::sprite,
};
use ge_core::mission::Difficulty;
use ge_render::frontend::Layer;

pub fn layers(assets: &Assets, snapshot: &Snapshot) -> Vec<Layer> {
    let mut out = Vec::new();
    sprite(&mut out, 1, [0., 0., 640., 480.], [1.; 4]);
    sprite(&mut out, 0, [0., 0., 640., 480.], [0., 0.03, 0.04, 0.88]);
    sprite(&mut out, 0, [0., 60., 640., 1.], [0.15, 0.24, 0.27, 0.8]);
    sprite(&mut out, 0x8600002b, [305., 40., 335., 7.], WHITE);
    sprite(&mut out, 0x86000029, [305., 48., 335., 17.], WHITE);
    sprite(&mut out, 0x8600002b, [305., 65., 335., 3.], WHITE);

    assets.text(&mut out, "Tutorial", 106., 96., 32., WHITE);
    // The controller admits only typed0..3 difficulty requests. A malformed
    // display snapshot never substitutes a source difficulty or exposes IDs.
    if let Ok(difficulty) = Difficulty::try_from(snapshot.difficulty) {
        assets.text(&mut out, difficulty.label(), 106., 139., 21., BLUE);
    }

    let (heading, lines, footer): (&str, &[&str], &str) = match snapshot.stage {
        SessionStage::Admitting => (
            "Starting New Game",
            &["Preparing your game..."],
            "Press Back to return to the menu.",
        ),
        SessionStage::Loading => (
            "Loading mission",
            &["Preparing the tutorial..."],
            "Press Back to return to the menu.",
        ),
        SessionStage::AwaitingWorld => (
            "Mission data loaded",
            &[
                "World initialization is incomplete.",
                "Gameplay is not available yet.",
            ],
            "Return to the menu to continue exploring.",
        ),
        SessionStage::AwaitingPlayer => (
            if snapshot
                .world_initialization
                .as_ref()
                .is_some_and(|world| world.player_initialization.is_some())
            {
                "Preparing player"
            } else {
                "World initialized"
            },
            &[
                "Player initialization is incomplete.",
                "Gameplay is not available yet.",
            ],
            "Return to the menu to continue exploring.",
        ),
        SessionStage::Failed => (
            "Could not start the mission",
            &["Please return to the menu", "and try again."],
            "Return to the menu to choose New Game again.",
        ),
    };
    sprite(
        &mut out,
        0,
        [106., 191., 470., 145.],
        [0.04, 0.10, 0.13, 0.92],
    );
    assets.text(&mut out, heading, 120., 206., 25., WHITE);
    for (index, line) in lines.iter().enumerate() {
        assets.text(&mut out, line, 120., 253. + index as f32 * 25., 19., BLUE);
    }
    sprite(&mut out, 0x8600002b, [106., 346., 470., 2.], WHITE);
    assets.text(&mut out, footer, 106., 355., 17., WHITE);
    assets.text(&mut out, "Esc  Back", 106., 399., 18., WHITE);
    sprite(&mut out, 0x8600002b, [106., 429., 470., 2.], WHITE);
    out
}

/// Same reference footer target and letterbox mapping as the normal menu.
pub fn hit_back(position: [f64; 2], size: [u32; 2]) -> bool {
    let scale = (f64::from(size[0]) / 640.).min(f64::from(size[1]) / 480.);
    if scale <= 0. || !position.iter().all(|value| value.is_finite()) {
        return false;
    }
    let x = (position[0] - (f64::from(size[0]) - 640. * scale) * 0.5) / scale;
    let y = (position[1] - (f64::from(size[1]) - 480. * scale) * 0.5) / scale;
    (106. ..230.).contains(&x) && (396. ..424.).contains(&y)
}

#[cfg(test)]
#[path = "tests/view.rs"]
mod tests;
