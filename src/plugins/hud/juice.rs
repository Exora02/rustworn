//! HUD juice: floor/biome indicator, score text, pickup popups.
//!
//! Floor indicator: a Text2d entity at top-center showing "Floor N — BIOME".
//! Score text: a Text2d next to the XP bar showing "Score: N".
//! Pickup popups: world-space Text2d entities that float up and fade.
//!
//! The floor indicator and score text are persistent HUD elements
//! (spawned once, positioned each frame by hud_update_system).
//! Pickup popups are spawned ad-hoc via a system that reads
//! a resource-based queue (the pickup_collect_system pushes to it).

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{JustifyText, TextLayout};

use crate::components::{
    HudElement, HudElementKind, PickupKind, PickupPopup,
};

use super::HUD_Z;

// ─── Biome name lookup ────────────────────────────────────────────────────

const BIOME_NAMES: &[&str] = &[
    "FORGOTTEN CATACOMBS",
    "BLOODFALL CAVERNS",
    "MAGMA CORE",
    "CRYSTAL THRONE",
    "VOID SANCTUM",
];

/// Return a human-readable biome name for the given index.
/// Falls back to "UNKNOWN" if the index is out of range.
pub fn biome_name(index: u32) -> &'static str {
    BIOME_NAMES.get(index as usize).copied().unwrap_or("UNKNOWN")
}

// ─── Floor indicator ──────────────────────────────────────────────────────

pub const FLOOR_OFFSET_Y: f32 = -90.0;
pub const FLOOR_FONT_SIZE: f32 = 18.0;

/// Spawn the floor indicator Text2d (called from spawn_hud in mod.rs).
pub fn spawn_floor_indicator(commands: &mut Commands) {
    commands.spawn((
        Text2d::new("Floor 0 — FORGOTTEN CATACOMBS"),
        TextFont {
            font_size: FLOOR_FONT_SIZE,
            ..default()
        },
        TextColor(Color::srgb(0.60, 0.85, 0.95)),
        Anchor::Center,
        TextLayout::new_with_justify(JustifyText::Center),
        Transform::from_xyz(0.0, 0.0, HUD_Z + 0.5),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::FloorIndicator,
        },
    ));
}

// ─── Score text ───────────────────────────────────────────────────────────

pub const SCORE_TEXT_OFFSET_Y: f32 = -10.0;
pub const SCORE_TEXT_OFFSET_X: f32 = 125.0; // Right of the XP bar
pub const SCORE_FONT_SIZE: f32 = 14.0;

/// Spawn the "Score: N" text label next to the XP bar.
pub fn spawn_score_text(commands: &mut Commands) {
    commands.spawn((
        Text2d::new("Score: 0"),
        TextFont {
            font_size: SCORE_FONT_SIZE,
            ..default()
        },
        TextColor(Color::srgb(0.95, 0.75, 0.20)),
        Anchor::CenterLeft,
        TextLayout::new_with_justify(JustifyText::Left),
        Transform::from_xyz(0.0, 0.0, HUD_Z + 0.5),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::ScoreText,
        },
    ));
}

/// Update the score text content and position. Called from hud_update_system.
/// We can't use `Query<&mut Text2d, With<HudElementKind>>>` because
/// HudElementKind isn't a component — it's a field on HudElement. We
/// use `Query<(&mut Text2d, &HudElement)>` instead.
///
/// This is integrated into hud_update_system in bars.rs.

// ─── Pickup popups ────────────────────────────────────────────────────────

/// Push a pickup popup to the queue. Called from pickup_collect_system
/// in pickups.rs after applying the pickup effect.
pub fn push_pickup_popup(
    commands: &mut Commands,
    kind: PickupKind,
    pos: Vec2,
) {
    let text = match kind {
        PickupKind::Gold(a) => format!("+{} Gold", a),
        PickupKind::Heal => "+1 HP".to_string(),
        PickupKind::DamageUp => "Damage Up!".to_string(),
        PickupKind::FireRateUp => "Fire Rate Up!".to_string(),
        PickupKind::SpeedUp => "Speed Up!".to_string(),
        PickupKind::Regen => "Regen!".to_string(),
        PickupKind::Weapon(w) => format!("{:?}", w),
    };

    let color = match kind {
        PickupKind::Gold(_) => Color::srgb(1.0, 0.85, 0.20),
        PickupKind::Heal => Color::srgb(0.30, 1.0, 0.30),
        PickupKind::Weapon(_) => Color::srgb(0.85, 0.60, 1.0),
        PickupKind::DamageUp => Color::srgb(1.0, 0.40, 0.20),
        PickupKind::FireRateUp => Color::srgb(0.30, 0.85, 0.95),
        PickupKind::SpeedUp => Color::srgb(0.30, 1.0, 0.60),
        PickupKind::Regen => Color::srgb(0.30, 0.95, 0.40),
    };

    commands.spawn((
        PickupPopup {
            lifetime: Timer::from_seconds(1.2, TimerMode::Once),
            drift_speed: 40.0,
        },
        Text2d::new(text),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(color),
        Anchor::Center,
        Transform::from_xyz(pos.x, pos.y, 1.5),
        Visibility::Inherited,
    ));
}

/// Per-frame update for live pickup popups: tick lifetime, drift up,
/// fade alpha, despawn on finish. Same pattern as damage_number_update
/// in fx.rs.
pub fn pickup_popup_update(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut PickupPopup, &mut Transform, &mut TextColor)>,
) {
    for (entity, mut popup, mut tf, mut color) in &mut q {
        popup.lifetime.tick(time.delta());
        let t = if popup.lifetime.duration().as_secs_f32() > 0.0 {
            (popup.lifetime.elapsed_secs() / popup.lifetime.duration().as_secs_f32())
                .clamp(0.0, 1.0)
        } else {
            1.0
        };
        tf.translation.y += popup.drift_speed * time.delta_secs();
        let alpha = (1.0 - t).powf(0.7);
        color.0 = color.0.with_alpha(alpha);
        if popup.lifetime.finished() {
            commands.entity(entity).despawn();
        }
    }
}