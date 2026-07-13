//! In-run shop overlay system.
//!
//! When the player enters a shop room, a Text2d overlay shows items
//! and prices. The player presses 1-4 to buy, ESC to dismiss.
//! Items purchased are applied to RunState directly.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::components::{Health, Player, AimIndicator};
use crate::resources::RunState;

/// Marker component for the in-run shop overlay Text2d entity.
#[derive(Component)]
#[allow(dead_code)]
pub struct ShopOverlay;

/// The shop overlay item entries, mirrored from the spawned ShopItem entities.
/// Generated once when the player enters a shop room.
#[allow(dead_code)]
const SHOP_ITEM_DEFS: &[(u32, &str)] = &[
    (5, "Damage Up (+0.5 base_damage)"),
    (8, "Crit Up (+5% crit_chance)"),
    (10, "Speed Up (+10% move_speed_mult)"),
    (12, "Pierce (+1 pierce)"),
    (12, "Regen (+0.5 HP/s)"),
];

/// Spawn the shop overlay Text2d. Called when entering a shop room.
#[allow(dead_code)]
pub fn spawn_shop_overlay(commands: &mut Commands, room_center: Vec2) {
    let mut lines = String::from("=== SHOP ===\n\n");
    for (i, (cost, desc)) in SHOP_ITEM_DEFS.iter().enumerate() {
        lines.push_str(&format!("{}: [{}g] {}\n", i + 1, cost, desc));
    }
    lines.push_str("\nPress 1-4 to buy  |  ESC to leave");

    commands.spawn((
        Text2d::new(lines),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.95, 0.85, 0.30)),
        Anchor::Center,
        TextLayout::new_with_justify(JustifyText::Center),
        Transform::from_xyz(room_center.x, room_center.y, 10.0),
        ShopOverlay,
        Visibility::Inherited,
    ));
}

/// Handle key presses for the shop overlay.
/// 1-4: buy the corresponding item (if enough gold)
/// ESC: despawn the overlay
#[allow(dead_code)]
pub fn shop_overlay_input_system(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut run: ResMut<RunState>,
    mut health_q: Query<&mut Health, (With<Player>, Without<AimIndicator>)>,
    overlay_q: Query<Entity, With<ShopOverlay>>,
) {
    let Ok(overlay_entity) = overlay_q.single() else {
        return;
    };

    if keys.just_pressed(KeyCode::Escape) {
        commands.entity(overlay_entity).despawn();
        return;
    }

    let slot = if keys.just_pressed(KeyCode::Digit1) {
        Some(0usize)
    } else if keys.just_pressed(KeyCode::Digit2) {
        Some(1)
    } else if keys.just_pressed(KeyCode::Digit3) {
        Some(2)
    } else if keys.just_pressed(KeyCode::Digit4) {
        Some(3)
    } else {
        None
    };

    if let Some(idx) = slot {
        if idx >= SHOP_ITEM_DEFS.len() {
            return;
        }
        let (cost, _desc) = SHOP_ITEM_DEFS[idx];
        if run.gold < cost {
            return;
        }
        run.gold = run.gold.saturating_sub(cost);
        match idx {
            0 => run.base_damage += 0.5,
            1 => run.crit_chance = (run.crit_chance + 0.05).min(1.0),
            2 => run.move_speed_mult += 0.1,
            3 => run.pierce += 1,
            _ => run.hp_regen_per_sec += 0.5,
        }
        if let Ok(mut h) = health_q.single_mut() {
            h.current = (h.current + 1.0).min(h.max);
        }
        log::info!(
            "Shop: bought item {} for {} gold ({} remaining)",
            idx + 1,
            cost,
            run.gold
        );
        commands.entity(overlay_entity).despawn();
    }
}