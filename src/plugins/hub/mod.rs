//! Gungeon-style visual hub world — replaces the plain text MainMenu
//! with 5 interactive colored zones on a dark stone floor.
//!
//! Zones:
//!   1. ENTER THE DEPTHS   (center, green/blue)  → start run
//!   2. CROWN SHOP         (left, gold)          → show shop overlay
//!   3. UNLOCKABLES        (right, purple)       → progress view
//!   4. CHARACTER SELECT   (top-left, cyan)      → change class
//!   5. STATS              (top-right, white)    → run history
//!
//! Spawned on OnEnter(MainMenu), despawned on OnExit(MainMenu).

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{JustifyText, TextLayout};
use log::info;

use crate::components::AppState;
use crate::resources::{CrownSave, MetaConfig, RunState};

use crate::plugins::settings::SettingsOpen;
use crate::plugins::hud::HUD_Z;

/// Z-offset for hub entities — just below the HUD.
const HUB_Z: f32 = HUD_Z - 0.5;
/// Z-offset for text on top of zone rectangles.
const HUB_TEXT_Z: f32 = HUB_Z + 0.1;

/// Size of each zone rectangle.
const ZONE_W: f32 = 200.0;
const ZONE_H: f32 = 50.0;

/// Larger zone for the main "ENTER THE DEPTHS" portal.
const PORTAL_W: f32 = 280.0;
const PORTAL_H: f32 = 70.0;

/// Background tile floor dimensions.
const FLOOR_W: f32 = 1280.0;
const FLOOR_H: f32 = 720.0;

/// Z-offset for overlay entities — above the hub, below the HUD.
const OVERLAY_Z: f32 = HUB_Z + 0.3;
const OVERLAY_TEXT_Z: f32 = OVERLAY_Z + 0.1;

/// Marker component identifying a hub zone entity.
#[derive(Component)]
pub struct HubZone {
    pub kind: HubZoneKind,
    pub base_color: Color,
}

/// Marker for the border behind a hub zone.
#[derive(Component)]
pub struct HubZoneBorder {
    pub kind: HubZoneKind,
}

/// Marker for the pulsing glow behind the EnterDepths zone.
#[derive(Component)]
pub struct HubZoneGlow;

/// Marker for the "RUSTWORN" title text entity.
#[derive(Component)]
pub struct HubTitle;

/// Marker for floating background ember particles.
#[derive(Component)]
pub struct HubParticle {
    /// Horizontal drift speed (can be negative for leftward drift).
    pub drift_x: f32,
    /// Upward drift speed.
    pub drift_y: f32,
    /// Intensity offset for color phase.
    pub phase: f32,
}

/// Marker for small sparkle dots near the portal.
#[derive(Component)]
pub struct PortalSparkle {
    /// Timer controlling visibility toggling.
    pub timer: Timer,
    /// Base position relative to the portal center.
    pub offset: Vec2,
}

/// Marker component identifying a hub overlay entity (shop, unlockables, etc.).
/// Used so ESC can despawn overlays without despawning the hub itself.
#[derive(Component)]
pub struct HubOverlay;

/// Which zone this entity represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubZoneKind {
    EnterDepths,
    CrownShop,
    Unlockables,
    CharacterSelect,
    Stats,
}

/// Plugin that manages the visual hub menu.
pub struct HubPlugin;

impl Plugin for HubPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::MainMenu), spawn_hub_menu)
            .add_systems(OnExit(AppState::MainMenu), despawn_hub_menu)
            .add_systems(
                Update,
                hub_title_animation_system.run_if(in_state(AppState::MainMenu)),
            )
            .add_systems(
                Update,
                hub_interaction_system.run_if(in_state(AppState::MainMenu)),
            );
    }
}

/// A background entity marker so we can despawn it with the rest.
#[derive(Component)]
struct HubBackground;

/// Spawn the hub world: dark stone floor background + 5 interactable zones.
fn spawn_hub_menu(mut commands: Commands) {
    info!("Spawning hub world");

    // ── Dark stone floor background ──────────────────────────────────
    commands.spawn((
        Sprite {
            color: Color::srgb(0.12, 0.12, 0.14),
            custom_size: Some(Vec2::new(FLOOR_W, FLOOR_H)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, HUB_Z - 0.1),
        HubBackground,
    ));

    // Subtle grid lines — horizontal and vertical lines to suggest
    // a stone tile floor.
    for x in [-400.0, -200.0, 0.0, 200.0, 400.0] {
        commands.spawn((
            Sprite {
                color: Color::srgba(0.2, 0.2, 0.25, 0.3),
                custom_size: Some(Vec2::new(2.0, FLOOR_H)),
                anchor: Anchor::Center,
                ..default()
            },
            Transform::from_xyz(x, 0.0, HUB_Z - 0.09),
            HubBackground,
        ));
    }
    for y in [-300.0, -150.0, 0.0, 150.0, 300.0] {
        commands.spawn((
            Sprite {
                color: Color::srgba(0.2, 0.2, 0.25, 0.3),
                custom_size: Some(Vec2::new(FLOOR_W, 2.0)),
                anchor: Anchor::Center,
                ..default()
            },
            Transform::from_xyz(0.0, y, HUB_Z - 0.09),
            HubBackground,
        ));
    }

    // ── Zone definitions ─────────────────────────────────────────────
    let zones: [(HubZoneKind, &str, (f32, f32), Color, f32, f32); 5] = [
        (
            HubZoneKind::EnterDepths,
            "ENTER THE DEPTHS",
            (0.0, 0.0),
            Color::srgb(0.1, 0.5, 0.4),
            PORTAL_W,
            PORTAL_H,
        ),
        (
            HubZoneKind::CrownShop,
            "CROWN SHOP",
            (-360.0, 0.0),
            Color::srgb(0.6, 0.5, 0.1),
            ZONE_W,
            ZONE_H,
        ),
        (
            HubZoneKind::Unlockables,
            "UNLOCKABLES",
            (360.0, 0.0),
            Color::srgb(0.4, 0.2, 0.5),
            ZONE_W,
            ZONE_H,
        ),
        (
            HubZoneKind::CharacterSelect,
            "CHARACTER SELECT",
            (-360.0, 100.0),
            Color::srgb(0.1, 0.5, 0.5),
            ZONE_W,
            ZONE_H,
        ),
        (
            HubZoneKind::Stats,
            "STATS",
            (360.0, 100.0),
            Color::srgb(0.5, 0.5, 0.5),
            ZONE_W,
            ZONE_H,
        ),
    ];

    for (kind, label, (x, y), color, w, h) in &zones {
        let zone_z = HUB_Z;
        let is_portal = *kind == HubZoneKind::EnterDepths;

        // Lighter tint for borders: 1.3x brighter
        let srgba = color.to_srgba();
        let lighter = Color::srgb(
            (srgba.red * 1.3).min(1.0),
            (srgba.green * 1.3).min(1.0),
            (srgba.blue * 1.3).min(1.0),
        );

        // Zone border (2px larger, 1.3x lighter, behind zone body)
        if !is_portal {
            // Only spawn explicit borders for non-portal zones
            // (portal has its own glow entity)
            commands.spawn((
                Sprite {
                    color: lighter,
                    custom_size: Some(Vec2::new(*w + 4.0, *h + 4.0)),
                    anchor: Anchor::Center,
                    ..default()
                },
                Transform::from_xyz(*x, *y, zone_z - 0.02),
                HubZoneBorder { kind: *kind },
            ));
        }

        // Portal glow behind EnterDepths
        if is_portal {
            commands.spawn((
                Sprite {
                    color: Color::srgba(0.3, 0.8, 0.7, 0.7),
                    custom_size: Some(Vec2::new(*w + 16.0, *h + 16.0)),
                    anchor: Anchor::Center,
                    ..default()
                },
                Transform::from_xyz(*x, *y, zone_z - 0.03),
                HubZoneGlow,
            ));

            // Brighter portal border
            commands.spawn((
                Sprite {
                    color: Color::srgba(0.3, 0.8, 0.7, 0.4),
                    custom_size: Some(Vec2::new(*w + 8.0, *h + 8.0)),
                    anchor: Anchor::Center,
                    ..default()
                },
                Transform::from_xyz(*x, *y, zone_z - 0.01),
                HubZone { kind: *kind, base_color: *color },
            ));
        }

        // Zone rectangle
        commands.spawn((
            Sprite {
                color: *color,
                custom_size: Some(Vec2::new(*w, *h)),
                anchor: Anchor::Center,
                ..default()
            },
            Transform::from_xyz(*x, *y, zone_z),
            HubZone { kind: *kind, base_color: *color },
        ));

        // Text label + subtitle
        let sub_text = match kind {
            HubZoneKind::EnterDepths => "Press ENTER to start run",
            HubZoneKind::CrownShop => "Buy items",
            HubZoneKind::Unlockables => "View progress",
            HubZoneKind::CharacterSelect => "Change class",
            HubZoneKind::Stats => "Run history",
        };

        let font_size = if *kind == HubZoneKind::EnterDepths {
            18.0
        } else {
            14.0
        };

        commands.spawn((
            Text2d::new(format!("{}\\n{}", label, sub_text)),
            TextFont {
                font_size,
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.95, 0.95)),
            Anchor::Center,
            TextLayout::new_with_justify(JustifyText::Center),
            Transform::from_xyz(*x, *y, HUB_TEXT_Z),
            HubZone { kind: *kind, base_color: Color::WHITE }, // text entities don't use base_color for shading
        ));

        // Keyboard hint below the zone
        let key_hint = match kind {
            HubZoneKind::EnterDepths => "[ENTER]",
            HubZoneKind::CrownShop => "[1]",
            HubZoneKind::Unlockables => "[2]",
            HubZoneKind::CharacterSelect => "[3]",
            HubZoneKind::Stats => "[4]",
        };
        commands.spawn((
            Text2d::new(key_hint),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(Color::srgba(0.7, 0.7, 0.7, 0.6)),
            Anchor::Center,
            TextLayout::new_with_justify(JustifyText::Center),
            Transform::from_xyz(*x, *y - *h / 2.0 - 14.0, HUB_TEXT_Z),
            HubZone { kind: *kind, base_color: Color::WHITE },
        ));
    }

    // ── ESC hint at bottom center ─────────────────────────────────────
    commands.spawn((
        Text2d::new("ESC to quit"),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgba(0.5, 0.5, 0.5, 0.7)),
        Anchor::Center,
        Transform::from_xyz(0.0, -300.0, HUB_TEXT_Z),
        HubBackground,
    ));

    // ── "RUSTWORN" title text ─────────────────────────────────────────
    // Large rust/gold-colored title at the top-center of the screen.
    // Animated by hub_title_animation_system (sin-based pulsing scale).
    commands.spawn((
        HubTitle,
        Text2d::new("RUSTWORN"),
        TextFont {
            font_size: 48.0,
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.55, 0.15)),
        Anchor::Center,
        TextLayout::new_with_justify(JustifyText::Center),
        Transform::from_xyz(0.0, 200.0, HUB_TEXT_Z + 0.05),
    ));

    // ── Floating ember particles ──────────────────────────────────────
    // 24 small colored circles drifting upward in the background.
    // Colours: oranges, golds, warm yellows. Respawn at bottom when
    // they drift off the top of the screen.
    let ember_colors = [
        Color::srgb(0.9, 0.5, 0.1),   // deep orange
        Color::srgb(0.95, 0.65, 0.2), // golden orange
        Color::srgb(0.8, 0.4, 0.2),   // rust
        Color::srgb(1.0, 0.8, 0.3),   // warm gold
        Color::srgb(0.85, 0.6, 0.15), // amber
    ];
    for i in 0..24 {
        let phase = i as f32 * 0.73;
        let px = (i as f32 * 97.3) % FLOOR_W - FLOOR_W / 2.0;
        let py = (i as f32 * 53.7) % FLOOR_H - FLOOR_H / 2.0;
        let drift_x = (phase * 1.7).sin() * 8.0; // slight horizontal wobble
        let drift_y = 12.0 + (i as f32 * 3.7) % 10.0; // upward speed 12-22
        let color = ember_colors[i % ember_colors.len()];
        commands.spawn((
            HubParticle {
                drift_x,
                drift_y,
                phase,
            },
            Sprite {
                color,
                custom_size: Some(Vec2::splat(4.0)),
                ..default()
            },
            Transform::from_xyz(px, py, HUB_Z - 0.08),
            Visibility::Inherited,
        ));
    }

    // ── Portal sparkles ───────────────────────────────────────────────
    // Small glowing dots near the ENTER THE DEPTHS zone that fade
    // in/out randomly.
    let sparkle_offsets = [
        Vec2::new(-160.0, -40.0),
        Vec2::new(160.0, -40.0),
        Vec2::new(-140.0, 0.0),
        Vec2::new(140.0, 0.0),
        Vec2::new(-100.0, 35.0),
        Vec2::new(100.0, 35.0),
        Vec2::new(0.0, -50.0),
    ];
    for (i, offset) in sparkle_offsets.iter().enumerate() {
        let t = i as f32 * 0.9;
        commands.spawn((
            PortalSparkle {
                timer: Timer::from_seconds(0.6 + (i as f32 * 0.15), TimerMode::Repeating),
                offset: *offset,
            },
            Sprite {
                color: Color::srgba(0.4, 0.95, 0.9, 0.0), // teal/white, starts invisible
                custom_size: Some(Vec2::splat(3.0)),
                ..default()
            },
            Transform::from_xyz(offset.x, offset.y, HUB_Z - 0.07),
            Visibility::Inherited,
        ));
    }

    info!("Hub world spawned — 5 zones + floor + title + particles");
}

/// Despawn all hub entities when leaving MainMenu.
fn despawn_hub_menu(
    mut commands: Commands,
    zones: Query<Entity, Or<(With<HubZone>, With<HubBackground>, With<HubOverlay>, With<HubZoneBorder>, With<HubZoneGlow>, With<HubTitle>, With<HubParticle>, With<PortalSparkle>)>>,
) {
    for entity in &zones {
        commands.entity(entity).despawn();
    }
}

// ─── Overlay spawn helpers ──────────────────────────────────────────────

/// Build the Crown Shop overlay text content.
fn build_shop_text(meta: &MetaConfig, save: &CrownSave) -> String {
    let mut text = format!("=== CROWN SHOP ===\n\nCrown: {}\n\n", save.crown);
    for (i, def) in meta.unlockables.iter().enumerate().take(8) {
        let status = if save.unlocks[i] { "✓" } else { "-" };
        let line = format!("{}: {:<16} — {:>3} Cr  [{}]\n", i + 1, def.name, def.base_cost, status);
        text.push_str(&line);
    }
    text.push_str("\n(More items available in runs)\n");
    text.push_str("\nPress ESC to close");
    text
}

/// Build the Unlockables overlay text content.
fn build_unlockables_text(meta: &MetaConfig, save: &CrownSave) -> String {
    let mut text = String::from("=== UNLOCKABLES ===\n\n");
    let categories = [
        ("Weapons", 0..6),
        ("Passives", 6..16),
        ("Actives", 16..20),
        ("Core", 20..25),
    ];
    for (cat_name, range) in &categories {
        text.push_str(&format!("{}:\n", cat_name));
        for i in range.clone() {
            let def = &meta.unlockables[i];
            let status = if save.unlocks[i] { "UNLOCKED" } else { "LOCKED" };
            text.push_str(&format!("  {:<20} — {:>3} Cr [{status}]\n", def.name, def.base_cost));
        }
        text.push('\n');
    }
    text.push_str("Press ESC to close");
    text
}

/// Build the Character Select overlay text content.
fn build_character_select_text(run: &RunState) -> String {
    let current = &run.selected_class;
    format!(
        "=== CHARACTER SELECT ===\n\n\
        Current: {:#?}\n\n\
        1: Marauder  [{}]  — Knife AoE | Grenade (3s)\n\
        2: Reaver    [{}]  — Lifesteal | Dash-strike (2s)\n\
        3: Spectre   [{}]  — Regen     | Teleport (1.5s)\n\
        4: Engineer  [{}]  — +1 HP     | Turret (4s)\n\n\
        Press 1-4 to select  |  ESC to close",
        current,
        if *current == crate::components::PlayerClass::Marauder { "selected" } else { "press 1" },
        if *current == crate::components::PlayerClass::Reaver { "selected" } else { "press 2" },
        if *current == crate::components::PlayerClass::Spectre { "selected" } else { "press 3" },
        if *current == crate::components::PlayerClass::Engineer { "selected" } else { "press 4" },
    )
}

/// Build the Stats overlay text content.
fn build_stats_text(run: &RunState) -> String {
    format!(
        "=== STATS ===\n\n\
        Runs completed: (not yet tracked)\n\
        Best floor:     N/A\n\
        Total kills:    {}\n\
        Gold collected: {}\n\
        Time played:    {:.1}s",
        run.enemies_killed, run.gold, run.time_elapsed,
    )
}

/// Spawn a dark background + multi-line text overlay centered on screen.
fn spawn_overlay(commands: &mut Commands, text: String) {
    // Semi-transparent dark background box
    commands.spawn((
        Sprite {
            color: Color::srgba(0.05, 0.05, 0.10, 0.90),
            custom_size: Some(Vec2::new(600.0, 450.0)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, OVERLAY_Z),
        HubOverlay,
    ));

    // Text
    commands.spawn((
        Text2d::new(text),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
        Anchor::Center,
        TextLayout::new_with_justify(JustifyText::Center),
        Transform::from_xyz(0.0, 0.0, OVERLAY_TEXT_Z),
        HubOverlay,
    ));
}

// ─── Title animation system ────────────────────────────────────────────

/// Animates hub title particles, title pulsing, and portal sparkles.
/// Runs every frame during MainMenu.
fn hub_title_animation_system(
    time: Res<Time>,
    mut title_q: Query<&mut Transform, With<HubTitle>>,
    mut particle_q: Query<(&mut HubParticle, &mut Transform, &mut Sprite)>,
    mut sparkle_q: Query<(&mut PortalSparkle, &mut Sprite)>,
) {
    // ── Title pulsing ───────────────────────────────────────────────
    // Subtle breathing: scale oscillates 1.0 ↔ 1.05 using sin(time).
    if let Ok(mut title_tf) = title_q.single_mut() {
        let pulse = 1.0 + (time.elapsed_secs() * 2.0).sin() * 0.025;
        title_tf.scale = Vec3::splat(pulse);
    }

    // ── Floating ember particles ────────────────────────────────────
    let half_w = FLOOR_W / 2.0;
    let half_h = FLOOR_H / 2.0;
    for (mut particle, mut tf, mut sprite) in &mut particle_q {
        // Drift upward + horizontal wobble
        tf.translation.x += particle.drift_x * time.delta_secs();
        tf.translation.y += particle.drift_y * time.delta_secs();

        // Subtle alpha pulse per particle
        let alpha = 0.5 + ((time.elapsed_secs() * 1.5 + particle.phase) * 1.3).sin() * 0.3;
        sprite.color.set_alpha(alpha.clamp(0.2, 0.8));

        // Respawn at bottom when off-screen top
        if tf.translation.y > half_h {
            tf.translation.y = -half_h;
            // Randomize x slightly for variety
            tf.translation.x = ((particle.phase + time.elapsed_secs() * 0.1) * 97.3) % half_w - half_w / 2.0;
        }
        // If drifted too far left or right, wrap
        if tf.translation.x > half_w {
            tf.translation.x = -half_w;
        } else if tf.translation.x < -half_w {
            tf.translation.x = half_w;
        }
    }

    // ── Portal sparkles ────────────────────────────────────────────
    for (mut sparkle, mut sprite) in &mut sparkle_q {
        sparkle.timer.tick(time.delta());
        if sparkle.timer.just_finished() {
            // Toggle visibility: random fade-in/out
            let current_alpha = sprite.color.alpha();
            if current_alpha < 0.1 {
                // Fade in
                sprite.color = sprite.color.with_alpha(0.6 + (sparkle.offset.x.abs() % 0.3));
            } else {
                // Fade out
                sprite.color = sprite.color.with_alpha(0.0);
            }
        }
    }
}

// ─── Interaction system ─────────────────────────────────────────────────

/// Handle mouse click on hub zones and keyboard Enter/Escape.
///
/// Keyboard 1-4 zone selection and Enter/Escape are also handled by
/// `menu_input_system` in setup.rs (which handles character select
/// and shop purchases). We handle Enter/Escape here too for visual
/// parity; the overlapping handlers are idempotent.
fn hub_interaction_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform), With<crate::components::MainCamera>>,
    zones: Query<(&HubZone, &Transform), (With<HubZone>, Without<HubBackground>)>,
    mut zone_sprites: Query<(&HubZone, &mut Sprite), (With<HubZone>, Without<HubBackground>)>,
    mut glow_q: Query<&mut Sprite, With<HubZoneGlow>>,
    overlays: Query<Entity, With<HubOverlay>>,
    run: Res<RunState>,
    meta: Res<MetaConfig>,
    save: Res<CrownSave>,
    mut next_state: ResMut<NextState<AppState>>,
    mut commands: Commands,
    mut settings_open: ResMut<SettingsOpen>,
) {
    let overlay_open = !overlays.is_empty();

    // ── Portal glow pulse ───────────────────────────────────────────
    for mut glow_sprite in &mut glow_q {
        let pulse = (time.elapsed_secs() * 2.0).sin() * 0.3 + 0.7;
        glow_sprite.color.set_alpha(pulse);
    }

    // ── Zone hover highlight ────────────────────────────────────────
    // Check mouse cursor against each zone's transform. If hovering,
    // brighten the zone sprite by 1.3x. Restore to base_color when not.
    let mut hovered_zone: Option<HubZoneKind> = None;
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_q.single() else {
        return;
    };
    if let Some(cursor_pos) = window.cursor_position() {
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
            for (zone, transform) in &zones {
                let zone_size = if zone.kind == HubZoneKind::EnterDepths {
                    Vec2::new(PORTAL_W, PORTAL_H)
                } else {
                    Vec2::new(ZONE_W, ZONE_H)
                };
                let half = zone_size / 2.0;
                let pos = transform.translation.truncate();
                if world_pos.x >= pos.x - half.x
                    && world_pos.x <= pos.x + half.x
                    && world_pos.y >= pos.y - half.y
                    && world_pos.y <= pos.y + half.y
                {
                    hovered_zone = Some(zone.kind);
                    break;
                }
            }
        }
    }

    // ── Apply hover color to zone body sprites ──────────────────────
    if overlay_open {
        // When overlay is open, restore all zones to base color.
        for (zone, mut sprite) in &mut zone_sprites {
            sprite.color = zone.base_color;
        }
    } else if let Some(hovered) = hovered_zone {
        for (zone, mut sprite) in &mut zone_sprites {
            if zone.kind == hovered {
                let bc = zone.base_color.to_srgba();
                sprite.color = Color::srgb(
                    (bc.red * 1.3).min(1.0),
                    (bc.green * 1.3).min(1.0),
                    (bc.blue * 1.3).min(1.0),
                );
            } else {
                sprite.color = zone.base_color;
            }
        }
    } else {
        // No hover, restore all.
        for (zone, mut sprite) in &mut zone_sprites {
            sprite.color = zone.base_color;
        }
    }

    // ESC: if settings are open, close them. If another overlay is open,
    // close it. Otherwise open the settings menu.
    if keys.just_pressed(KeyCode::Escape) {
        if settings_open.0 {
            settings_open.0 = false;
            info!("Settings overlay closed");
            return;
        }
        if overlay_open {
            for entity in &overlays {
                commands.entity(entity).despawn();
            }
            info!("Hub overlay closed");
            return;
        } else {
            settings_open.0 = true;
            info!("Settings overlay opened");
            return;
        }
    }

    // Enter → start run (only if no overlay is open)
    if keys.just_pressed(KeyCode::Enter) {
        if overlay_open {
            return;
        }
        info!("Hub: ENTER THE DEPTHS");
        next_state.set(AppState::InRun);
        return;
    }

    // ── Mouse click detection ────────────────────────────────────────
    if mouse.just_pressed(MouseButton::Left) {
        let Ok(window) = windows.single() else {
            return;
        };
        let Ok((camera, camera_transform)) = camera_q.single() else {
            return;
        };

        let Some(cursor_pos) = window.cursor_position() else {
            return;
        };
        let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
            return;
        };

        for (zone, transform) in &zones {
            let zone_size = if zone.kind == HubZoneKind::EnterDepths {
                Vec2::new(PORTAL_W, PORTAL_H)
            } else {
                Vec2::new(ZONE_W, ZONE_H)
            };
            let half = zone_size / 2.0;
            let pos = transform.translation.truncate();
            if world_pos.x >= pos.x - half.x
                && world_pos.x <= pos.x + half.x
                && world_pos.y >= pos.y - half.y
                && world_pos.y <= pos.y + half.y
            {
                info!("Hub: clicked {:?}", zone.kind);
                act_on_zone(zone.kind, &meta, &save, &run, next_state, &mut commands);
                return;
            }
        }
    }
}

/// Perform the action associated with a hub zone.
fn act_on_zone(
    kind: HubZoneKind,
    meta: &MetaConfig,
    save: &CrownSave,
    run: &RunState,
    mut next_state: ResMut<NextState<AppState>>,
    commands: &mut Commands,
) {
    match kind {
        HubZoneKind::EnterDepths => {
            next_state.set(AppState::InRun);
        }
        HubZoneKind::CrownShop => {
            info!("=== CROWN SHOP ===");
            let text = build_shop_text(meta, save);
            spawn_overlay(commands, text);
        }
        HubZoneKind::Unlockables => {
            info!("=== UNLOCKABLES ===");
            let text = build_unlockables_text(meta, save);
            spawn_overlay(commands, text);
        }
        HubZoneKind::CharacterSelect => {
            info!("=== CHARACTER SELECT ===");
            let text = build_character_select_text(run);
            spawn_overlay(commands, text);
        }
        HubZoneKind::Stats => {
            info!("=== STATS ===");
            let text = build_stats_text(run);
            spawn_overlay(commands, text);
        }
    }
}