//! Reference-canvas menu composition and matching host hit targets.
use crate::frontend::{Assets, BLUE, WHITE};
use ge_core::frontend::{Action, Screen, State};
use ge_render::frontend::{Layer, Quad};

pub fn layers(assets: &Assets, state: &State, elapsed: f64) -> Vec<Layer> {
    let mut q = Vec::new();
    sprite(&mut q, 1, [0., 0., 640., 480.], [1.; 4]);
    sprite(&mut q, 0, [0., 0., 640., 480.], [0., 0.03, 0.04, 0.17]);
    // Static reference-derived composition. Original script timing remains open.
    sprite(&mut q, 0, [0., 60., 640., 1.], [0.15, 0.24, 0.27, 0.8]);
    sprite(&mut q, 0, [0., 0., 640., 480.], [0., 0., 0., 0.10]);
    sprite(&mut q, 0x8600002b, [305., 40., 335., 7.], WHITE);
    sprite(&mut q, 0x86000029, [305., 48., 335., 17.], [1.; 4]);
    sprite(&mut q, 0x8600002b, [305., 65., 335., 3.], WHITE);
    // The two pivots and linking arm follow the captured Wii composition.
    circle(&mut q, 133., 70., 28., WHITE);
    circle(&mut q, 133., 70., 22., [0.04, 0.12, 0.16, 1.]);
    circle(&mut q, 133., 70., 18., BLUE);
    circle(&mut q, 133., 70., 6., WHITE);
    line(&mut q, [136., 76.], [185., 141.], 5., WHITE);
    circle(&mut q, 185., 140., 19., BLUE);
    circle(&mut q, 185., 140., 7., WHITE);
    circle(&mut q, 185., 140., 3., [0.05, 0.10, 0.13, 1.]);
    assets.text(&mut q, state.title(), 160., 38., 25., WHITE);
    assets.text(&mut q, "Profile:", 160., 73., 23., WHITE);
    assets.text(&mut q, "Guest", 218., 73., 23., BLUE);
    match state.screen() {
        Screen::Unavailable(action) => unavailable(assets, &mut q, action),
        Screen::Credits => credits(assets, &mut q, elapsed),
        _ => rows(assets, &mut q, state),
    }
    sprite(&mut q, 0x8600002b, [106., 346., 470., 2.], WHITE);
    sprite(&mut q, 0x8600002b, [106., 429., 470., 2.], WHITE);
    assets.text(&mut q, "Esc  Back", 106., 399., 18., WHITE);
    if matches!(
        state.screen(),
        Screen::SoundSettings | Screen::ScreenSettings
    ) {
        assets.text(&mut q, "R  Reset", 295., 399., 18., WHITE);
        assets.text(&mut q, "<  Adjust  >", 470., 399., 18., WHITE);
    } else if !matches!(state.screen(), Screen::Credits | Screen::Unavailable(_)) {
        assets.text(&mut q, "Select  Enter", 473., 399., 18., WHITE);
    }
    q
}
pub(crate) fn sprite(out: &mut Vec<Layer>, texture: u32, rect: [f32; 4], color: [f32; 4]) {
    out.push(
        Quad {
            texture,
            rect,
            uv: [0., 0., 1., 1.],
            color,
        }
        .into(),
    );
}

fn rows(assets: &Assets, out: &mut Vec<Layer>, state: &State) {
    for (index, item) in state.items().iter().enumerate() {
        let y = 110. + index as f32 * 27.5;
        let selected = index == state.selected_index();
        let color = if !item.enabled {
            [0.40, 0.45, 0.47, 1.]
        } else if selected {
            BLUE
        } else {
            WHITE
        };
        sprite(
            out,
            0,
            [213., y, 370., 25.],
            if selected {
                [0.98, 0.99, 0.98, 1.]
            } else {
                [0.27, 0.39, 0.45, 0.72]
            },
        );
        assets.text(out, item.label, 220., y + 1., 23., color);
        if let Some(value) = item.value {
            let progress = if item.action == Action::Brightness {
                f32::from(value - 25) / 100.
            } else {
                f32::from(value) / 100.
            };
            sprite(out, 0, [443., y + 19., 125., 2.], [0.09, 0.18, 0.23, 0.6]);
            sprite(out, 0, [443., y + 19., 125. * progress, 2.], color);
            assets.text(out, "<", 431., y + 2., 18., color);
            assets.text(out, &format!("{value}"), 480., y + 1., 18., color);
            assets.text(out, ">", 570., y + 2., 18., color);
        }
    }
    let mut height = 17.;
    while height > 10.
        && paragraph_rows(assets, state.description(), 470., height) as f32 * (height + 2.) > 38.
    {
        height -= 0.5;
    }
    paragraph(
        assets,
        out,
        state.description(),
        [106., 355., 470.],
        height,
        WHITE,
    );
}

fn paragraph_rows(assets: &Assets, text: &str, width: f32, height: f32) -> usize {
    let mut rows = 1;
    let mut used = 0.;
    let space = text_width(&assets.font, " ", height);
    for word in text.split_whitespace() {
        let next = text_width(&assets.font, word, height);
        if used > 0. && used + space + next > width {
            rows += 1;
            used = next;
        } else {
            used += if used == 0. { next } else { space + next };
        }
    }
    rows
}

fn paragraph(
    assets: &Assets,
    out: &mut Vec<Layer>,
    text: &str,
    bounds: [f32; 3],
    height: f32,
    color: [f32; 4],
) {
    let [x, y, width] = bounds;
    let mut line = String::new();
    let mut line_width = 0.;
    let mut y = y;
    for word in text.split_whitespace() {
        let word_width = text_width(&assets.font, word, height);
        let space = if line.is_empty() {
            0.
        } else {
            text_width(&assets.font, " ", height)
        };
        if !line.is_empty() && line_width + space + word_width > width {
            assets.text(out, &line, x, y, height, color);
            y += height + 2.;
            line.clear();
            line_width = 0.;
        }
        if !line.is_empty() {
            line.push(' ');
            line_width += space;
        }
        line.push_str(word);
        line_width += word_width;
    }
    if !line.is_empty() {
        assets.text(out, &line, x, y, height, color);
    }
}
fn text_width(font: &ge_assets::frontend_font::FrontendFont, text: &str, height: f32) -> f32 {
    text.encode_utf16()
        .filter_map(|code| font.body.glyph(code))
        .map(|g| f32::from(g.width) * height / f32::from(g.height.max(1)))
        .sum()
}
fn unavailable(assets: &Assets, out: &mut Vec<Layer>, action: Action) {
    sprite(out, 0, [213., 111., 370., 205.], [0.04, 0.10, 0.13, 0.92]);
    assets.text(out, action.label(), 226., 123., 27., WHITE);
    paragraph(
        assets,
        out,
        "This feature is not implemented yet.",
        [226., 175., 337.],
        20.,
        WHITE,
    );
    paragraph(
        assets,
        out,
        "Press Esc or Backspace to return.",
        [226., 250., 337.],
        18.,
        BLUE,
    );
}

fn credits(assets: &Assets, out: &mut Vec<Layer>, elapsed: f64) {
    // The native credits advance one pixel/update. This host adapts that to a
    // 60-Hz timebase; original viewport, scroll cadence and script parity are open.
    let source = assets.credits.lines();
    let padding = source
        .iter()
        .position(|line| !line.text.trim().is_empty())
        .unwrap_or(source.len());
    let source = &source[padding..];
    let scroll = (elapsed * 60.) as f32;
    let first = (((scroll - 225.) / 24.).max(0.) as usize)
        .saturating_sub(1)
        .min(source.len());
    let mut row_y = 334. - scroll + first as f32 * 24.;
    // Only the visible window needs text measurement and glyph construction.
    for line in source.iter().skip(first).take(13) {
        let y = row_y - if line.heading { 8. } else { 0. };
        let height = if line.heading { 28. } else { 20. };
        if y >= 109. && y + height <= 340. {
            let (font, texture) = if line.heading {
                (&assets.font, assets.font_texture)
            } else {
                (&assets.credits_font, assets.credits_texture)
            };
            let width = text_width(font, &line.text, height);
            let fit = (460. / width.max(1.)).min(1.);
            let mut x = 106. + (470. - width * fit) * 0.5;
            for code in line.text.encode_utf16() {
                if let Some(glyph) = font.body.glyph(code) {
                    let h = height * fit;
                    let w = f32::from(glyph.width) * h / f32::from(glyph.height.max(1));
                    out.push(
                        Quad {
                            texture,
                            rect: [x, y, w, h],
                            uv: [
                                glyph.uv_min[0],
                                glyph.uv_min[1],
                                glyph.uv_max[0],
                                glyph.uv_max[1],
                            ],
                            color: if line.heading { WHITE } else { BLUE },
                        }
                        .into(),
                    );
                    x += w;
                }
            }
        }
        row_y += 24.;
    }
    if 334. - scroll + source.len() as f32 * 24. < 109. {
        assets.text(out, "End of credits", 266., 215., 22., WHITE);
    }
}

/// Host brightness changes RGB modulation only. Preserve alpha, clip W,
/// geometry, texture state and the exact baseline path when brightness is 100.
pub fn brightness(layers: &mut [Layer], value: u8) {
    if value == 100 {
        return;
    }
    let gain = f32::from(value) / 100.;
    for layer in layers {
        match layer {
            Layer::Quad(quad) => {
                for lane in &mut quad.color[..3] {
                    *lane *= gain;
                }
            }
            Layer::Mesh(mesh) => {
                for vertex in &mut mesh.vertices {
                    for lane in &mut vertex.color[..3] {
                        *lane *= gain;
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Row(usize),
    Decrease(usize),
    Increase(usize),
    Back,
    Reset,
    Confirm,
}
/// Letterbox conversion and hit rectangles share this module with row layout.
pub fn hit(position: [f64; 2], size: [u32; 2], state: &State) -> Option<Hit> {
    let scale = (f64::from(size[0]) / 640.).min(f64::from(size[1]) / 480.);
    if scale <= 0. || !position.iter().all(|v| v.is_finite()) {
        return None;
    }
    let x = (position[0] - (f64::from(size[0]) - 640. * scale) * 0.5) / scale;
    let y = (position[1] - (f64::from(size[1]) - 480. * scale) * 0.5) / scale;
    if (396. ..424.).contains(&y) {
        return if (106. ..230.).contains(&x) {
            Some(Hit::Back)
        } else if matches!(
            state.screen(),
            Screen::SoundSettings | Screen::ScreenSettings
        ) && (275. ..415.).contains(&x)
        {
            Some(Hit::Reset)
        } else if (470. ..586.).contains(&x) {
            if matches!(
                state.screen(),
                Screen::SoundSettings | Screen::ScreenSettings
            ) {
                Some(if x < 528. {
                    Hit::Decrease(state.selected_index())
                } else {
                    Hit::Increase(state.selected_index())
                })
            } else {
                Some(Hit::Confirm)
            }
        } else {
            None
        };
    }
    if !(213. ..583.).contains(&x) || !(110. ..302.).contains(&y) {
        return None;
    }
    let row = ((y - 110.) / 27.5) as usize;
    let items = state.items();
    let item = items.get(row)?;
    if (y - 110.) % 27.5 >= 25. {
        return None;
    }
    if item.value.is_some() && x >= 425. {
        Some(if x < 510. {
            Hit::Decrease(row)
        } else {
            Hit::Increase(row)
        })
    } else {
        Some(Hit::Row(row))
    }
}

fn circle(out: &mut Vec<Layer>, x: f32, y: f32, r: f32, color: [f32; 4]) {
    sprite(out, 0x86000025, [x - r, y - r, r * 2., r * 2.], color);
}
fn line(out: &mut Vec<Layer>, a: [f32; 2], b: [f32; 2], width: f32, color: [f32; 4]) {
    // Small squares approximate the single static connector without a second pipeline.
    let steps = ((b[0] - a[0]).abs().max((b[1] - a[1]).abs())).ceil() as usize;
    for i in 0..=steps {
        let t = i as f32 / steps.max(1) as f32;
        sprite(
            out,
            0,
            [
                a[0] + (b[0] - a[0]) * t - width / 2.,
                a[1] + (b[1] - a[1]) * t - width / 2.,
                width,
                width,
            ],
            color,
        );
    }
}

#[cfg(test)]
#[path = "tests/view.rs"]
mod tests;
