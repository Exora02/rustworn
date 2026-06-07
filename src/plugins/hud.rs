//! HUD: HP bar, fire-rate indicator, kill counter, time, game-over overlay.
//!
//! Day 13-14: minimal in-world HUD that stays screen-stable by being
//! re-positioned each frame at a fixed offset from the player (since
//! the camera follows the player, this is equivalent to a screen-
//! space HUD without the cost of a second camera + UI layer).
//!
//! Day 15-16: kill counter and elapsed-time bars added at the top-
//! right (mirrored from HP + fire-rate on the top-left), and a
//! centered game-over overlay + restart prompt that toggle their
//! `Visibility` based on `RunState.game_over`.
//!
//! Each bar element is a bg+fill pair sharing the same `HudElementKind`.
//! The bg is the dark backdrop, the fill encodes state via
//! `Transform::scale.x` (0..1) and `Sprite.color`. We discriminate
//! the two sprites per kind by checking the color against the
//! known BG constant (a 0.01 epsilon on each channel).
//!
//! Game over / restart prompt use a single sprite with `Visibility` —
//! hidden by default, shown by `hud_update_system` when the run ends.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::components::{Health, HudElement, HudElementKind, Player, Weapon};
use crate::resources::RunState;

/// Width of the player's view in world units. The HUD layout uses
/// this to position the top-left / top-right pairs at the screen
/// edges regardless of the window resolution.
const VIEW_WIDTH: f32 = 1280.0;
const VIEW_HEIGHT: f32 = 720.0;

// ─── HP bar (top-left) ────────────────────────────────────────────────────

const HP_GREEN: Color = Color::srgb(0.30, 0.85, 0.30);
const HP_RED: Color = Color::srgb(0.85, 0.20, 0.20);
const HP_BG: Color = Color::srgb(0.10, 0.10, 0.10);
const HP_BAR_WIDTH: f32 = 120.0;
const HP_BAR_HEIGHT: f32 = 16.0;

// ─── Fire-rate bar (below HP) ────────────────────────────────────────────

const FIRE_RATE_COLOR: Color = Color::srgb(0.95, 0.85, 0.30);
const FIRE_RATE_BG: Color = Color::srgb(0.10, 0.10, 0.10);
const FIRE_RATE_WIDTH: f32 = 60.0;
const FIRE_RATE_HEIGHT: f32 = 8.0;

// ─── Kill counter (top-right) ────────────────────────────────────────────

const KILL_COLOR: Color = Color::srgb(0.95, 0.55, 0.20);
const KILL_BG: Color = Color::srgb(0.10, 0.10, 0.10);
const KILL_BAR_WIDTH: f32 = 80.0;
const KILL_BAR_HEIGHT: f32 = 10.0;
/// Number of kills that fills the kill bar. Bar caps here; the
/// actual count is encoded in `RunState.enemies_killed` and grows
/// unbounded.
const KILL_BAR_FILL_AT: u32 = 10;

// ─── Elapsed-time bar (below kills) ──────────────────────────────────────

const TIME_COLOR: Color = Color::srgb(0.30, 0.85, 0.85);
const TIME_BG: Color = Color::srgb(0.10, 0.10, 0.10);
const TIME_BAR_WIDTH: f32 = 80.0;
const TIME_BAR_HEIGHT: f32 = 6.0;
/// Time in seconds the bar takes to fill before it visually resets.
/// One minute is a natural readout — players can gauge survival
/// chunks without having to read a numeric counter.
const TIME_BAR_WINDOW_S: f32 = 60.0;

// ─── Game over overlay (screen center) ───────────────────────────────────

const GAME_OVER_COLOR: Color = Color::srgba(0.85, 0.10, 0.10, 0.90);
const GAME_OVER_WIDTH: f32 = 360.0;
const GAME_OVER_HEIGHT: f32 = 80.0;
const GAME_OVER_OFFSET_Y: f32 = -60.0;

const RESTART_PROMPT_COLOR: Color = Color::srgba(0.30, 0.85, 0.30, 0.90);
const RESTART_PROMPT_WIDTH: f32 = 240.0;
const RESTART_PROMPT_HEIGHT: f32 = 40.0;
const RESTART_PROMPT_OFFSET_Y: f32 = 50.0;

// ─── Layout offsets (top-left, top-right) ────────────────────────────────

const HUD_OFFSET: Vec2 = Vec2::new(-(VIEW_WIDTH * 0.5) + 60.0, VIEW_HEIGHT * 0.5 - 40.0);
const HUD_OFFSET_RIGHT: Vec2 = Vec2::new(
    VIEW_WIDTH * 0.5 - 60.0 - KILL_BAR_WIDTH,
    VIEW_HEIGHT * 0.5 - 40.0,
);
const FIRE_RATE_OFFSET_Y: f32 = -22.0;
const TIME_BAR_OFFSET_Y: f32 = -16.0;

/// Z value for HUD sprites — well above the player (0), enemies (0),
/// and projectiles (0.5), so the HUD always renders on top.
const HUD_Z: f32 = 10.0;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        // The HUD doesn't care about hitstop or game over — it keeps
        // updating through both so the player can see their HP drop
        // during a freeze and the game-over overlay can stay visible.
        app.add_systems(Startup, spawn_hud)
            .add_systems(Update, hud_update_system);
    }
}

fn spawn_hud(mut commands: Commands) {
    spawn_bar(
        &mut commands,
        HUD_OFFSET,
        HP_BAR_WIDTH,
        HP_BAR_HEIGHT,
        HP_BG,
        HP_GREEN,
        HudElementKind::HpBar,
    );
    spawn_bar(
        &mut commands,
        HUD_OFFSET + Vec2::new(0.0, FIRE_RATE_OFFSET_Y),
        FIRE_RATE_WIDTH,
        FIRE_RATE_HEIGHT,
        FIRE_RATE_BG,
        FIRE_RATE_COLOR,
        HudElementKind::FireRate,
    );
    spawn_bar(
        &mut commands,
        HUD_OFFSET_RIGHT,
        KILL_BAR_WIDTH,
        KILL_BAR_HEIGHT,
        KILL_BG,
        KILL_COLOR,
        HudElementKind::KillBar,
    );
    spawn_bar(
        &mut commands,
        HUD_OFFSET_RIGHT + Vec2::new(0.0, TIME_BAR_OFFSET_Y),
        TIME_BAR_WIDTH,
        TIME_BAR_HEIGHT,
        TIME_BG,
        TIME_COLOR,
        HudElementKind::TimeBar,
    );

    // Game-over overlay: a single semi-transparent red rectangle.
    // Hidden by default; `hud_update_system` flips its visibility
    // when `RunState.game_over` toggles.
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

    // Restart prompt: smaller green rectangle below the game over.
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
}

// Spawn a bg+fill pair sharing a HudElementKind. The bg has scale
// (1, 1) and never changes; the fill's scale.x encodes the value
// and its color is the per-bar "active" color. Both share the same
// position so they overlap exactly.
fn spawn_bar(
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

fn hud_update_system(
    // `Without<HudElement>` is a no-op in practice (the Player never
    // carries one) but it tells Bevy's disjointness checker that this
    // query is mutually exclusive with the `hud_query` below, which
    // needs `&mut Transform`. Without the hint, Bevy raises B0001
    // because both queries touch `Transform`.
    player_query: Query<(&Transform, &Health, &Weapon), (With<Player>, Without<HudElement>)>,
    run: Res<RunState>,
    mut hud_query: Query<(&mut Transform, &mut Sprite, &mut Visibility, &HudElement)>,
) {
    let Ok((player_tf, health, weapon)) = player_query.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();

    for (mut tf, mut sprite, mut vis, element) in &mut hud_query {
        match element.kind {
            HudElementKind::HpBar => {
                let pos = player_pos + HUD_OFFSET;
                tf.translation = pos.extend(tf.translation.z);
                let ratio = (health.current / health.max).clamp(0.0, 1.0);
                tf.scale.x = ratio;
                if !approx_eq_color(sprite.color, HP_BG) {
                    sprite.color = lerp_color(HP_RED, HP_GREEN, ratio);
                }
            }
            HudElementKind::FireRate => {
                let pos = player_pos + HUD_OFFSET + Vec2::new(0.0, FIRE_RATE_OFFSET_Y);
                tf.translation = pos.extend(tf.translation.z);
                let interval = 1.0 / weapon.fire_rate;
                let remaining = weapon.fire_timer.remaining_secs();
                let progress = ((interval - remaining) / interval).clamp(0.0, 1.0);
                tf.scale.x = progress;
                if !approx_eq_color(sprite.color, FIRE_RATE_BG) {
                    sprite.color = FIRE_RATE_COLOR;
                }
            }
            HudElementKind::KillBar => {
                let pos = player_pos + HUD_OFFSET_RIGHT;
                tf.translation = pos.extend(tf.translation.z);
                let ratio = (run.enemies_killed as f32 / KILL_BAR_FILL_AT as f32).clamp(0.0, 1.0);
                tf.scale.x = ratio;
                if !approx_eq_color(sprite.color, KILL_BG) {
                    sprite.color = KILL_COLOR;
                }
            }
            HudElementKind::TimeBar => {
                let pos = player_pos + HUD_OFFSET_RIGHT + Vec2::new(0.0, TIME_BAR_OFFSET_Y);
                tf.translation = pos.extend(tf.translation.z);
                // Time bar wraps every TIME_BAR_WINDOW_S seconds so the
                // visual stays in a useful 0..1 range. The HUD never
                // shows the absolute elapsed time; players who want the
                // exact count can divide the bar position by window.
                let progress = (run.time_elapsed / TIME_BAR_WINDOW_S).clamp(0.0, 1.0);
                tf.scale.x = progress;
                if !approx_eq_color(sprite.color, TIME_BG) {
                    sprite.color = TIME_COLOR;
                }
            }
            HudElementKind::GameOver => {
                // Position once on death and stay put — the camera is
                // frozen so the overlay sits dead-center on screen.
                tf.translation = Vec3::new(
                    player_pos.x,
                    player_pos.y + GAME_OVER_OFFSET_Y,
                    tf.translation.z,
                );
                *vis = if run.game_over {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            HudElementKind::RestartPrompt => {
                tf.translation = Vec3::new(
                    player_pos.x,
                    player_pos.y + RESTART_PROMPT_OFFSET_Y,
                    tf.translation.z,
                );
                *vis = if run.game_over {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

// Linear color interpolation in sRGB. Both inputs are srgb; we
// extract the (r, g, b) channels, lerp, and reconstruct. Alpha is
// passed through (HUD elements are always opaque).
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let a = a.to_srgba();
    let b = b.to_srgba();
    Color::srgba(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        a.alpha,
    )
}

// Cheap "is this color approximately equal to the given constant?"
// check. Used to discriminate between the background sprite and the
// fill sprite (which share the same HudElementKind but should not
// have their color updated). A 0.01 epsilon on each channel is
// generous for sRGB.
fn approx_eq_color(c: Color, target: Color) -> bool {
    let c = c.to_srgba();
    let t = target.to_srgba();
    (c.red - t.red).abs() < 0.01
        && (c.green - t.green).abs() < 0.01
        && (c.blue - t.blue).abs() < 0.01
}
