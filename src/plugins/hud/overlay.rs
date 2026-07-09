//! Game-over overlay with run stats breakdown.
//!
//! Two multi-line Text2d entities: a stats summary for death and one
//! for extraction. Each contains the title ("GAME OVER" / "EXTRACTED!"),
//! run stats (floors, kills, gold, time, crowns), and instructions.
//! A separate RestartPrompt entity sits below showing input options.
//!
//! The text content is updated dynamically each frame by
//! `hud_update_system` so the values reflect the final RunState.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{JustifyText, TextLayout};

use crate::components::{HudElement, HudElementKind};

use super::HUD_Z;

pub const GAME_OVER_BG_COLOR: Color = Color::srgba(0.20, 0.05, 0.05, 0.85);
pub const EXTRACT_BG_COLOR: Color = Color::srgba(0.05, 0.20, 0.08, 0.85);
/// Default text color for stats lines.
const STATS_TEXT_COLOR: Color = Color::srgb(0.85, 0.85, 0.85);

/// BG sprite size — large enough to cover the screen center area.
pub const GAME_OVER_BG_SIZE: (f32, f32) = (500.0, 320.0);

/// Z offsets for the bg sprite and text layers.
pub const BG_Z: f32 = HUD_Z + 0.2;
pub const TEXT_Z: f32 = HUD_Z + 0.3;

/// Placeholder text that gets replaced every frame during GameOver.
/// We need non-empty text so the Text2d entity has valid content at
/// spawn time (Bevy doesn't render empty text). The hud_update_system
/// overwrites this immediately when the state becomes GameOver.
const PLACEHOLDER_TEXT: &str = "GAME OVER\n\n\n\n\n";

/// Spawn the game-over stats overlays + restart prompt (called from
/// `spawn_hud` in mod.rs).
pub fn spawn_game_over_overlay(commands: &mut Commands) {
    // Death overlay: dim bg box (so the stats text is readable on
    // any background) + multi-line stats text.
    commands.spawn((
        Sprite {
            color: GAME_OVER_BG_COLOR,
            custom_size: Some(Vec2::new(GAME_OVER_BG_SIZE.0, GAME_OVER_BG_SIZE.1)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, BG_Z),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::GameOver,
        },
    ));
    commands.spawn((
        Text2d::new(PLACEHOLDER_TEXT),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(STATS_TEXT_COLOR),
        Anchor::Center,
        TextLayout::new_with_justify(JustifyText::Center),
        Transform::from_xyz(0.0, 0.0, TEXT_Z),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::GameOver,
        },
    ));

    // Extraction overlay: green-tinted bg + multi-line stats text.
    commands.spawn((
        Sprite {
            color: EXTRACT_BG_COLOR,
            custom_size: Some(Vec2::new(GAME_OVER_BG_SIZE.0, GAME_OVER_BG_SIZE.1)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, BG_Z),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::GameOverExtracted,
        },
    ));
    commands.spawn((
        Text2d::new(PLACEHOLDER_TEXT),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(STATS_TEXT_COLOR),
        Anchor::Center,
        TextLayout::new_with_justify(JustifyText::Center),
        Transform::from_xyz(0.0, 0.0, TEXT_Z),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::GameOverExtracted,
        },
    ));

    // Restart prompt: centered below the stats box.
    commands.spawn((
        Sprite {
            color: Color::srgba(0.30, 0.85, 0.30, 0.90),
            custom_size: Some(Vec2::new(280.0, 36.0)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(0.0, 200.0, BG_Z),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::RestartPrompt,
        },
    ));
    commands.spawn((
        Text2d::new("R to restart  |  ESC to exit"),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Anchor::Center,
        TextLayout::new_with_justify(JustifyText::Center),
        Transform::from_xyz(0.0, 200.0, TEXT_Z),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::RestartPrompt,
        },
    ));
}

/// Format the elapsed time (seconds) as "Mm Ss" — e.g., "2m 14s".
pub fn format_time(secs: f32) -> String {
    let total = secs as u32;
    let minutes = total / 60;
    let seconds = total % 60;
    format!("{}m {:02}s", minutes, seconds)
}

/// Format the stats text for the current RunState.
pub fn format_stats_text(run: &crate::resources::RunState, title: &str) -> String {
    let floors = run.biome_index;
    let kills = run.enemies_killed;
    let gold = run.gold;
    let time_str = format_time(run.time_elapsed);
    let crowns = floors * 12 + gold / 3;

    format!(
        "\
{title}
━━━━━━━━━━━━━━━━━━━━

Floors Cleared:  {floors}
Enemies Killed:  {kills}
Gold Collected:  {gold}
Time Survived:   {time_str}
Crowns Earned:   {crowns}",
        title = title,
        floors = floors,
        kills = kills,
        gold = gold,
        time_str = time_str,
        crowns = crowns,
    )
}