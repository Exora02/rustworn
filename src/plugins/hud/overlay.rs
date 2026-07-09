//! Game-over overlay and restart prompt.
//!
//! Two entities each: a colored box (sprite) and a text label.
//! They live on separate entities so the text can sit at a higher
//! z than the sprite (Bevy 2D sorts back-to-front by z, and same-z
//! is arbitrary — without the z offset the sprite would obscure
//! the text on at least some drivers). Both entities are positioned
//! + visibility-toggled by `hud_update_system`.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{JustifyText, TextLayout};

use crate::components::{HudElement, HudElementKind};

use super::HUD_Z;

pub const GAME_OVER_COLOR: Color = Color::srgba(0.85, 0.10, 0.10, 0.90);
pub const GAME_OVER_WIDTH: f32 = 360.0;
pub const GAME_OVER_HEIGHT: f32 = 80.0;
pub const GAME_OVER_OFFSET_Y: f32 = -60.0;
pub const GAME_OVER_TEXT: &str = "GAME OVER";
// Sized to fit comfortably in the 360×80 box with ~16px vertical
// breathing room. 48 = cap height ~36, full glyph height ~48, so
// the descender on "G" is the closest any letter gets to the
// bottom edge. 96 (the previous value) overflowed the box top
// and bottom — the text rendered, just got clipped/looked huge.
pub const GAME_OVER_FONT_SIZE: f32 = 48.0;

pub const RESTART_PROMPT_COLOR: Color = Color::srgba(0.30, 0.85, 0.30, 0.90);
pub const RESTART_PROMPT_WIDTH: f32 = 240.0;
pub const RESTART_PROMPT_HEIGHT: f32 = 40.0;
pub const RESTART_PROMPT_OFFSET_Y: f32 = 50.0;
pub const RESTART_TEXT: &str = "Press R to restart";
// 240×40 box → font_size 24 fits with ~8px vertical margin.
pub const RESTART_FONT_SIZE: f32 = 24.0;

/// Spawn the game-over overlay + restart prompt entities (called
/// from `spawn_hud` in mod.rs).
pub fn spawn_game_over_overlay(commands: &mut Commands) {
    // Game-over overlay. Two entities sharing the same
    // `HudElementKind::GameOver`: the colored box (sprite) and
    // the text label. They live on separate entities so the
    // text can sit at a higher z than the sprite (Bevy 2D
    // sorts back-to-front by z, and same-z is arbitrary —
    // without the z offset the sprite would obscure the text
    // on at least some drivers). Both entities are positioned
    // + visibility-toggled by `hud_update_system`.
    commands.spawn((
        Sprite {
            color: GAME_OVER_COLOR,
            custom_size: Some(Vec2::new(GAME_OVER_WIDTH, GAME_OVER_HEIGHT)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(0.0, GAME_OVER_OFFSET_Y, HUD_Z + 0.2),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::GameOver,
        },
    ));
    commands.spawn((
        Text2d::new(GAME_OVER_TEXT),
        TextFont {
            font_size: GAME_OVER_FONT_SIZE,
            ..default()
        },
        // Bright yellow on the red box — high contrast in case
        // the previous white-on-red was getting eaten by the
        // semi-transparent red overlay.
        TextColor(Color::srgb(1.0, 0.95, 0.20)),
        // Explicit `Anchor::Center` as a separate component
        // (in addition to the auto-required default) — the
        // `Text2d::new` constructor's `#[require(...)]` should
        // insert this automatically, but on the user's Bazzite
        // box the text isn't rendering at all even with the
        // z-offset fix, so we set it explicitly to rule out
        // a missing auto-requirement.
        // (See bevy-0.16-gotchas §12 correction note.)
        Anchor::Center,
        // Explicit `TextLayout` with center justification. The
        // default is left-aligned, which would put the text at
        // the left edge of the box even with `Anchor::Center`
        // on the entity. For "GAME OVER" (a short string) the
        // difference is small, but for "Press R to restart"
        // (longer) it's visible. Setting this explicitly
        // removes one variable from the "why isn't it
        // rendering" investigation.
        TextLayout::new_with_justify(JustifyText::Center),
        Transform::from_xyz(0.0, GAME_OVER_OFFSET_Y, HUD_Z + 0.3),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::GameOver,
        },
    ));

    // Restart prompt: same two-entity pattern (box + text) as
    // the game-over overlay. The two entities are independently
    // positioned by `hud_update_system` so they stay in lockstep.
    commands.spawn((
        Sprite {
            color: RESTART_PROMPT_COLOR,
            custom_size: Some(Vec2::new(RESTART_PROMPT_WIDTH, RESTART_PROMPT_HEIGHT)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(0.0, RESTART_PROMPT_OFFSET_Y, HUD_Z + 0.2),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::RestartPrompt,
        },
    ));
    commands.spawn((
        Text2d::new(RESTART_TEXT),
        TextFont {
            font_size: RESTART_FONT_SIZE,
            ..default()
        },
        TextColor(Color::WHITE),
        Anchor::Center,
        TextLayout::new_with_justify(JustifyText::Center),
        Transform::from_xyz(0.0, RESTART_PROMPT_OFFSET_Y, HUD_Z + 0.3),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::RestartPrompt,
        },
    ));
}
