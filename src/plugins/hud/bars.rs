//! HUD bars: HP, fire-rate, XP, kill counter, time, cascade, boss HP.
//!
//! Each bar element is a bg+fill pair sharing the same `HudElementKind`.
//! The bg is the dark backdrop, the fill encodes state via
//! `Transform::scale.x` (0..1) and `Sprite.color`. We discriminate
//! the two sprites per kind by checking the color against the
//! known BG constant (a 0.01 epsilon on each channel).

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::components::{
    Boss, Health, HudElement, HudElementKind, Player, Weapon, WeaponArchetype,
};
use crate::resources::{CascadeState, FloorState, RunState};

use super::{
    approx_eq_color, lerp_color, FIRE_RATE_OFFSET_Y, HUD_OFFSET, HUD_OFFSET_RIGHT, HUD_Z,
    TIME_BAR_OFFSET_Y,
};

// ─── HP bar (top-left) ────────────────────────────────────────────────────

pub const HP_GREEN: Color = Color::srgb(0.30, 0.85, 0.30);
pub const HP_RED: Color = Color::srgb(0.85, 0.20, 0.20);
pub const HP_BG: Color = Color::srgb(0.10, 0.10, 0.10);
pub const HP_BAR_WIDTH: f32 = 120.0;
pub const HP_BAR_HEIGHT: f32 = 16.0;

// ─── Fire-rate bar (below HP) ────────────────────────────────────────────

pub const FIRE_RATE_BG: Color = Color::srgb(0.10, 0.10, 0.10);
pub const FIRE_RATE_PISTOL: Color = Color::srgb(0.95, 0.85, 0.30);
pub const FIRE_RATE_SHOTGUN: Color = Color::srgb(0.95, 0.30, 0.30);
pub const FIRE_RATE_BURST: Color = Color::srgb(0.30, 0.85, 0.95);
pub const FIRE_RATE_BEAM: Color = Color::srgb(1.0, 0.60, 0.10);
pub const FIRE_RATE_WIDTH: f32 = 60.0;
pub const FIRE_RATE_HEIGHT: f32 = 8.0;

// ─── XP bar (above kill bar) ──────────────────────────────────────────────

pub const XP_BAR_FILL_AT: u32 = 10;
pub const XP_BAR_WIDTH: f32 = 120.0;
pub const XP_BAR_HEIGHT: f32 = 8.0;
pub const XP_BAR_BG: Color = Color::srgb(0.10, 0.10, 0.10);
pub const XP_BAR_COLOR: Color = Color::srgb(0.95, 0.75, 0.20);
pub const XP_BAR_OFFSET_Y: f32 = -10.0;

// ─── Kill counter bar (top-right) ─────────────────────────────────────────

pub const KILL_COLOR: Color = Color::srgb(0.95, 0.55, 0.20);
pub const KILL_BG: Color = Color::srgb(0.10, 0.10, 0.10);
pub const KILL_BAR_WIDTH: f32 = 80.0;
pub const KILL_BAR_HEIGHT: f32 = 10.0;
pub const KILL_BAR_FILL_AT: u32 = 10;

// ─── Elapsed-time bar (below kills) ──────────────────────────────────────

pub const TIME_COLOR: Color = Color::srgb(0.30, 0.85, 0.85);
pub const TIME_BG: Color = Color::srgb(0.10, 0.10, 0.10);
pub const TIME_BAR_WIDTH: f32 = 80.0;
pub const TIME_BAR_HEIGHT: f32 = 6.0;
pub const TIME_BAR_WINDOW_S: f32 = 60.0;

// ─── Cascade bar (below time) ────────────────────────────────────────────

pub const CASCADE_COLOR: Color = Color::srgb(0.45, 0.20, 0.85);
pub const CASCADE_BG: Color = Color::srgb(0.10, 0.10, 0.10);
pub const CASCADE_BAR_WIDTH: f32 = 60.0;
pub const CASCADE_BAR_HEIGHT: f32 = 8.0;
pub const CASCADE_BAR_OFFSET_Y: f32 = -14.0;

// ─── Boss HP bar (top-center) ────────────────────────────────────────────

pub const BOSS_HP_BAR_COLOR: Color = Color::srgb(0.90, 0.20, 0.20);
pub const BOSS_HP_BAR_BG: Color = Color::srgb(0.10, 0.10, 0.10);
pub const BOSS_HP_BAR_BORDER: Color = Color::srgb(0.30, 0.30, 0.30);
pub const BOSS_HP_BAR_WIDTH: f32 = 200.0;
pub const BOSS_HP_BAR_HEIGHT: f32 = 16.0;
pub const BOSS_HP_BAR_OFFSET_Y: f32 = -90.0;
pub const BOSS_LABEL_OFFSET_Y: f32 = -100.0;
pub const BOSS_LABEL_TEXT: &str = "BOSS";

/// Color the fire-rate bar fill by the current weapon archetype.
/// Pistol reads as a calm yellow, shotgun as a hot red, burst
/// as a cool cyan, beam as a hot orange — each weapon gets a
/// distinct arcade color so the bar is a quick "what weapon am
/// I on" readout. The beam's orange matches the beam sprite
/// in the world for a consistent "energy weapon" signal.
pub fn fire_rate_color(archetype: WeaponArchetype) -> Color {
    match archetype {
        WeaponArchetype::Pistol => FIRE_RATE_PISTOL,
        WeaponArchetype::Shotgun => FIRE_RATE_SHOTGUN,
        WeaponArchetype::Burst => FIRE_RATE_BURST,
        WeaponArchetype::Beam => FIRE_RATE_BEAM,
        WeaponArchetype::RocketLauncher => FIRE_RATE_BEAM,
    }
}

/// Spawn a bg + fill bar pair at the given position.
pub fn spawn_bar(
    commands: &mut Commands,
    pos: Vec2,
    width: f32,
    height: f32,
    bg_color: Color,
    fill_color: Color,
    kind: HudElementKind,
) {
    commands.spawn((
        Sprite {
            color: bg_color,
            custom_size: Some(Vec2::new(width, height)),
            anchor: Anchor::CenterLeft,
            ..default()
        },
        Transform::from_xyz(pos.x, pos.y, HUD_Z),
        HudElement { kind },
    ));
    commands.spawn((
        Sprite {
            color: fill_color,
            custom_size: Some(Vec2::new(width, height)),
            anchor: Anchor::CenterLeft,
            ..default()
        },
        Transform::from_xyz(pos.x, pos.y, HUD_Z + 0.1),
        HudElement { kind },
    ));
}

/// Phase 1 mini-boss: spawn the top-center boss HP bar as a
/// bg + fill pair (same pattern as `spawn_bar` but the fill
/// is queried separately in `hud_update_system` to update the
/// scale to the boss's current HP). The bar's anchor is
/// `Center` so the translation is the bar's center, and the
/// y-offset in `hud_update_system` puts it above the player
/// (top of the screen). Hidden by default; flipped to
/// `Visibility::Inherited` by `hud_update_system` while a
/// `With<Boss>` entity is alive. Also spawns a 1px border
/// outline and a "BOSS" text label above the bar.
pub fn spawn_boss_hp_bar(commands: &mut Commands) {
    commands.spawn((
        Sprite {
            color: BOSS_HP_BAR_BORDER,
            custom_size: Some(Vec2::new(BOSS_HP_BAR_WIDTH + 2.0, BOSS_HP_BAR_HEIGHT + 2.0)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 10.0),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::BossHpBar,
        },
    ));
    commands.spawn((
        Sprite {
            color: BOSS_HP_BAR_BG,
            custom_size: Some(Vec2::new(BOSS_HP_BAR_WIDTH, BOSS_HP_BAR_HEIGHT)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 10.0),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::BossHpBar,
        },
    ));
    commands.spawn((
        Sprite {
            color: BOSS_HP_BAR_COLOR,
            custom_size: Some(Vec2::new(BOSS_HP_BAR_WIDTH, BOSS_HP_BAR_HEIGHT)),
            anchor: Anchor::CenterLeft,
            ..default()
        },
        Transform::from_xyz(
            -(BOSS_HP_BAR_WIDTH * 0.5),
            0.0,
            10.0 + 0.1,
        ),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::BossHpBar,
        },
    ));
    // "BOSS" label text above the bar
    commands.spawn((
        Text2d::new(BOSS_LABEL_TEXT),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgb(0.95, 0.90, 0.90)),
        TextLayout::new_with_justify(JustifyText::Center),
        Transform::from_xyz(0.0, BOSS_LABEL_OFFSET_Y, 10.0 + 0.2),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::BossHpBar,
        },
    ));
}

/// Spawn all bar HUD elements (called from `spawn_hud` in mod.rs).
pub fn spawn_all_bars(commands: &mut Commands) {
    spawn_bar(
        commands,
        HUD_OFFSET,
        HP_BAR_WIDTH,
        HP_BAR_HEIGHT,
        HP_BG,
        HP_GREEN,
        HudElementKind::HpBar,
    );
    spawn_bar(
        commands,
        HUD_OFFSET + Vec2::new(0.0, FIRE_RATE_OFFSET_Y),
        FIRE_RATE_WIDTH,
        FIRE_RATE_HEIGHT,
        FIRE_RATE_BG,
        FIRE_RATE_PISTOL,
        HudElementKind::FireRate,
    );
    spawn_bar(
        commands,
        HUD_OFFSET_RIGHT + Vec2::new(0.0, XP_BAR_OFFSET_Y),
        XP_BAR_WIDTH,
        XP_BAR_HEIGHT,
        XP_BAR_BG,
        XP_BAR_COLOR,
        HudElementKind::XpBar,
    );
    spawn_bar(
        commands,
        HUD_OFFSET_RIGHT,
        KILL_BAR_WIDTH,
        KILL_BAR_HEIGHT,
        KILL_BG,
        KILL_COLOR,
        HudElementKind::KillBar,
    );
    spawn_bar(
        commands,
        HUD_OFFSET_RIGHT + Vec2::new(0.0, TIME_BAR_OFFSET_Y),
        TIME_BAR_WIDTH,
        TIME_BAR_HEIGHT,
        TIME_BG,
        TIME_COLOR,
        HudElementKind::TimeBar,
    );
    // Phase 1 cascade: top-right, below the time bar. Bar fill
    // semantics flip between cooldown and active: during cooldown
    // it fills as the next window approaches, during the window
    // it drains as time runs out. Hidden entirely between Cascade
    // cycles — but the cascade cycle is always running in this
    // prototype, so it's effectively always visible. The bar
    // still hides on game over (no point showing "Cascade
    // incoming" while the player is dead).
    spawn_bar(
        commands,
        HUD_OFFSET_RIGHT + Vec2::new(0.0, CASCADE_BAR_OFFSET_Y),
        CASCADE_BAR_WIDTH,
        CASCADE_BAR_HEIGHT,
        CASCADE_BG,
        CASCADE_COLOR,
        HudElementKind::CascadeBar,
    );
    // Phase 1 mini-boss: top-center of the screen, 60% of the
    // viewport wide, 8px tall. Hidden by default; the HUD update
    // system flips it to `Visibility::Inherited` only when a
    // `With<Boss>` entity exists in the world, and back to
    // `Visibility::Hidden` when the boss is killed. The bar
    // sits ABOVE the player (y > HUD_OFFSET_Y) so it reads as
    // a "boss is here" indicator, not a player stat.
    spawn_boss_hp_bar(commands);
}

/// Update all HUD bar elements each frame. Positions each bar
/// relative to the player (camera follows player → screen-stable).
pub fn hud_update_system(
    player_query: Query<(&Transform, &Health, &Weapon), (With<Player>, Without<HudElement>)>,
    run: Res<RunState>,
    floor_state: Res<FloorState>,
    cascade: Res<CascadeState>,
    mut hud_query: Query<(
        &mut Transform,
        Option<&mut Sprite>,
        &mut Visibility,
        &HudElement,
        Option<&mut Text2d>,
    )>,
    boss_alive: Query<(), With<Boss>>,
    boss_health: Query<&Health, With<Boss>>,
) {
    let Ok((player_tf, health, weapon)) = player_query.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();
    let boss_alive_global_flag = if boss_alive.is_empty() {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };

    for (mut tf, sprite, mut vis, element, text) in &mut hud_query {
        match element.kind {
            HudElementKind::MainMenu => {
                let _ = (&mut tf, sprite, &mut vis);
            }
            HudElementKind::HpBar => {
                let pos = player_pos + HUD_OFFSET;
                tf.translation = pos.extend(tf.translation.z);
                let ratio = (health.current / health.max).clamp(0.0, 1.0);
                tf.scale.x = ratio;
                if let Some(mut sprite) = sprite {
                    if !approx_eq_color(sprite.color, HP_BG) {
                        sprite.color = lerp_color(HP_RED, HP_GREEN, ratio);
                    }
                }
            }
            HudElementKind::FireRate => {
                let pos = player_pos + HUD_OFFSET + Vec2::new(0.0, FIRE_RATE_OFFSET_Y);
                tf.translation = pos.extend(tf.translation.z);
                let interval = 1.0 / weapon.fire_rate;
                let remaining = weapon.fire_timer.remaining_secs();
                let progress = ((interval - remaining) / interval).clamp(0.0, 1.0);
                tf.scale.x = progress;
                if let Some(mut sprite) = sprite {
                    if !approx_eq_color(sprite.color, FIRE_RATE_BG) {
                        sprite.color = fire_rate_color(weapon.archetype);
                    }
                }
            }
            HudElementKind::XpBar => {
                let pos = player_pos + HUD_OFFSET_RIGHT + Vec2::new(0.0, XP_BAR_OFFSET_Y);
                tf.translation = pos.extend(tf.translation.z);
                let ratio = (run.xp as f32 / XP_BAR_FILL_AT as f32).clamp(0.0, 1.0);
                tf.scale.x = ratio;
                if let Some(mut sprite) = sprite {
                    if !approx_eq_color(sprite.color, XP_BAR_BG) {
                        sprite.color = XP_BAR_COLOR;
                    }
                }
            }
            HudElementKind::KillBar => {
                let pos = player_pos + HUD_OFFSET_RIGHT;
                tf.translation = pos.extend(tf.translation.z);
                let ratio = (run.enemies_killed as f32 / KILL_BAR_FILL_AT as f32).clamp(0.0, 1.0);
                tf.scale.x = ratio;
                if let Some(mut sprite) = sprite {
                    if !approx_eq_color(sprite.color, KILL_BG) {
                        sprite.color = KILL_COLOR;
                    }
                }
            }
            HudElementKind::TimeBar => {
                let pos = player_pos + HUD_OFFSET_RIGHT + Vec2::new(0.0, TIME_BAR_OFFSET_Y);
                tf.translation = pos.extend(tf.translation.z);
                let progress = (run.time_elapsed / TIME_BAR_WINDOW_S).clamp(0.0, 1.0);
                tf.scale.x = progress;
                if let Some(mut sprite) = sprite {
                    if !approx_eq_color(sprite.color, TIME_BG) {
                        sprite.color = TIME_COLOR;
                    }
                }
            }
            HudElementKind::CascadeBar => {
                let pos = player_pos + HUD_OFFSET_RIGHT + Vec2::new(0.0, CASCADE_BAR_OFFSET_Y);
                tf.translation = pos.extend(tf.translation.z);
                let progress = if cascade.active {
                    let dur = cascade.duration_timer.duration().as_secs_f32();
                    if dur > 0.0 {
                        (cascade.duration_timer.remaining_secs() / dur).clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                } else {
                    let dur = cascade.interval_timer.duration().as_secs_f32();
                    if dur > 0.0 {
                        (cascade.interval_timer.elapsed_secs() / dur).clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                };
                tf.scale.x = progress;
                if let Some(mut sprite) = sprite {
                    if !approx_eq_color(sprite.color, CASCADE_BG) {
                        sprite.color = CASCADE_COLOR;
                    }
                }
            }
            HudElementKind::GameOver => {
                tf.translation = Vec3::new(player_pos.x, player_pos.y, tf.translation.z);
                if let Some(mut text) = text {
                    let title = if run.game_over_reason == crate::components::GameOverReason::Died {
                        "[ GAME OVER ]"
                    } else {
                        "[ EXTRACTED! ]"
                    };
                    text.0 = super::overlay::format_stats_text(&run, title);
                }
            }
            HudElementKind::GameOverExtracted => {
                tf.translation = Vec3::new(player_pos.x, player_pos.y, tf.translation.z);
                if let Some(mut text) = text {
                    let title = if run.game_over_reason == crate::components::GameOverReason::Died {
                        "[ GAME OVER ]"
                    } else {
                        "[ EXTRACTED! ]"
                    };
                    text.0 = super::overlay::format_stats_text(&run, title);
                }
            }
            HudElementKind::RestartPrompt => {
                tf.translation = Vec3::new(player_pos.x, player_pos.y + 200.0, tf.translation.z);
            }
            HudElementKind::BossHpBar => {
                tf.translation = Vec3::new(
                    player_pos.x,
                    player_pos.y + BOSS_HP_BAR_OFFSET_Y,
                    tf.translation.z,
                );
                *vis = boss_alive_global_flag;
                // Update fill sprite scale from boss HP ratio
                if let Some(sprite) = sprite {
                    if !approx_eq_color(sprite.color, BOSS_HP_BAR_BG)
                        && !approx_eq_color(sprite.color, BOSS_HP_BAR_BORDER)
                    {
                        // Fill sprite — update scale from boss HP
                        if let Ok(boss_hp) = boss_health.single() {
                            let ratio = (boss_hp.current / boss_hp.max).clamp(0.0, 1.0);
                            tf.scale.x = ratio;
                        }
                    }
                }
            }
            HudElementKind::FloorIndicator => {
                tf.translation = Vec3::new(
                    player_pos.x,
                    player_pos.y + super::juice::FLOOR_OFFSET_Y,
                    tf.translation.z,
                );
                if let Some(mut text) = text {
                    let name = super::juice::biome_name(run.biome_index);
                    text.0 = format!("Floor {} — {}", floor_state.floor_number + 1, name);
                }
            }
            HudElementKind::ScoreText => {
                let pos = player_pos
                    + HUD_OFFSET_RIGHT
                    + Vec2::new(
                        super::juice::SCORE_TEXT_OFFSET_X,
                        super::juice::SCORE_TEXT_OFFSET_Y,
                    );
                tf.translation = pos.extend(tf.translation.z);
                if let Some(mut text) = text {
                    text.0 = format!("Score: {}", run.xp);
                }
            }
            // Meta shop: screen-stable at fixed offset from player
            // (only visible in MainMenu, where the player is at origin).
            HudElementKind::MetaShop => {
                tf.translation = Vec3::new(
                    player_pos.x - 580.0,
                    player_pos.y + 320.0,
                    tf.translation.z,
                );
            }
            // Character select: screen-stable below player origin.
            HudElementKind::CharacterSelect => {
                tf.translation = Vec3::new(
                    player_pos.x,
                    player_pos.y - 280.0,
                    tf.translation.z,
                );
            }
            // Pause overlay: centered on screen.
            HudElementKind::PauseOverlay => {
                tf.translation = Vec3::new(player_pos.x, player_pos.y, tf.translation.z);
            }
        }
    }
}
