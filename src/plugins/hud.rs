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

use crate::components::{
    CascadeTelegraph, Health, HudElement, HudElementKind, Player, Weapon, WeaponArchetype,
};
use crate::resources::{CascadeState, RunState};

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

const FIRE_RATE_BG: Color = Color::srgb(0.10, 0.10, 0.10);
const FIRE_RATE_PISTOL: Color = Color::srgb(0.95, 0.85, 0.30);
const FIRE_RATE_SHOTGUN: Color = Color::srgb(0.95, 0.30, 0.30);
const FIRE_RATE_BURST: Color = Color::srgb(0.30, 0.85, 0.95);
const FIRE_RATE_BEAM: Color = Color::srgb(1.0, 0.60, 0.10);
const FIRE_RATE_WIDTH: f32 = 60.0;
const FIRE_RATE_HEIGHT: f32 = 8.0;

/// Color the fire-rate bar fill by the current weapon archetype.
/// Pistol reads as a calm yellow, shotgun as a hot red, burst
/// as a cool cyan, beam as a hot orange — each weapon gets a
/// distinct arcade color so the bar is a quick "what weapon am
/// I on" readout. The beam's orange matches the beam sprite
/// in the world for a consistent "energy weapon" signal.
fn fire_rate_color(archetype: WeaponArchetype) -> Color {
    match archetype {
        WeaponArchetype::Pistol => FIRE_RATE_PISTOL,
        WeaponArchetype::Shotgun => FIRE_RATE_SHOTGUN,
        WeaponArchetype::Burst => FIRE_RATE_BURST,
        WeaponArchetype::Beam => FIRE_RATE_BEAM,
    }
}

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

// ─── Cascade bar (below time) ────────────────────────────────────────────

const CASCADE_COLOR: Color = Color::srgb(0.45, 0.20, 0.85);
const CASCADE_BG: Color = Color::srgb(0.10, 0.10, 0.10);
const CASCADE_BAR_WIDTH: f32 = 60.0;
const CASCADE_BAR_HEIGHT: f32 = 8.0;
const CASCADE_BAR_OFFSET_Y: f32 = -14.0;

const GAME_OVER_COLOR: Color = Color::srgba(0.85, 0.10, 0.10, 0.90);
const GAME_OVER_WIDTH: f32 = 360.0;
const GAME_OVER_HEIGHT: f32 = 80.0;
const GAME_OVER_OFFSET_Y: f32 = -60.0;
const GAME_OVER_TEXT: &str = "GAME OVER";
const GAME_OVER_FONT_SIZE: f32 = 96.0;

const RESTART_PROMPT_COLOR: Color = Color::srgba(0.30, 0.85, 0.30, 0.90);
const RESTART_PROMPT_WIDTH: f32 = 240.0;
const RESTART_PROMPT_HEIGHT: f32 = 40.0;
const RESTART_PROMPT_OFFSET_Y: f32 = 50.0;
const RESTART_TEXT: &str = "Press R to restart";
const RESTART_FONT_SIZE: f32 = 32.0;

// ─── Cascade telegraph overlay (Phase 1) ──────────────────────────────
//
// A single full-screen sprite that tints the world during a
// Cascade window. The visual progression:
//
//   idle     — overlay hidden, world is normal
//   warning  — 3s before the next window opens, purple
//              pulses at ~2 Hz, alpha 0..MAX_PULSE
//   active   — window is open, solid purple tint at ACTIVE_ALPHA
//
// All three states use the same sprite entity; only the
// color/alpha/visibility change frame to frame.

/// Tint color for the cascade overlay — matches the cascade bar
/// in the HUD so the player sees the same purple both in the
/// corner indicator and across the whole world.
const CASCADE_TELEGRAPH_COLOR: Color = Color::srgb(0.45, 0.20, 0.85);

/// Alpha during the active window. Low enough that the world is
/// still readable (you need to see enemies through the tint) but
/// high enough that the cascade state is unmistakable.
const CASCADE_TELEGRAPH_ACTIVE_ALPHA: f32 = 0.30;

/// Peak alpha during the 3s warning pulse. Lower than the active
/// alpha so the warning reads as a "preparing to happen" signal
/// rather than the cascade itself being live.
const CASCADE_TELEGRAPH_PULSE_MAX_ALPHA: f32 = 0.18;

/// How many seconds before the cascade window opens the warning
/// pulse begins. 3s gives the player enough time to register
/// the visual change, prepare positioning, and plan a route
/// into the densest enemy cluster.
const CASCADE_TELEGRAPH_WARNING_LEAD_S: f32 = 3.0;

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
        app.add_systems(Startup, (spawn_hud, spawn_cascade_telegraph))
            .add_systems(
                Update,
                (cascade_telegraph_update, hud_update_system).chain(),
            )
            // ─── DIAGNOSTIC (feat/text2d-investigation) ───────────
            // A probe that runs once per second and logs whether the
            // default Font handle resolves to a real Font asset.
            // If `default_font` is missing from the bevy feature
            // list in Cargo.toml, the bound check will report false
            // every time — the smoking gun for the Text2d-rendering
            // bug. Removed by the user after the investigation.
            .add_systems(Update, text2d_probe_system);
    }
}

// ─── DIAGNOSTIC (feat/text2d-investigation) ─────────────────────────────────
// Runs every ~60 frames (~1s at 60fps) and logs:
//   - whether the default `Handle<Font>` is bound to a real Font asset
//   - the count of Text2d entities in the world
//   - whether a Text2d entity has a populated `TextLayoutInfo.glyphs` vec
//
// If the default Font handle is unbound, then the
// `default_font` feature is missing from Cargo.toml — that's
// the root cause. Throttled with a Local counter so it
// doesn't spam the log. We only check the asset-binding
// question (the most informative probe); the per-entity
// layout/glyph details require private bevy_text internals.
fn text2d_probe_system(
    mut frame_counter: Local<u32>,
    fonts: Res<Assets<Font>>,
    text_query: Query<Entity, With<Text2d>>,
) {
    *frame_counter += 1;
    if !(*frame_counter == 1 || *frame_counter % 60 == 0) {
        return;
    }
    let default_handle = Handle::<Font>::default();
    let bound = fonts.get(&default_handle).is_some();
    log::info!(
        "[text2d-probe] default Handle<Font> bound to a real Font asset: {} \
         (this MUST be true for Text2d to render anything — false means \
         `default_font` feature is missing from Cargo.toml)",
        bound
    );
    log::info!(
        "[text2d-probe] Text2d entity count: {}",
        text_query.iter().count()
    );
}
// ─── END DIAGNOSTIC ─────────────────────────────────────────────────────────

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
        FIRE_RATE_PISTOL,
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
    // Phase 1 cascade: top-right, below the time bar. Bar fill
    // semantics flip between cooldown and active: during cooldown
    // it fills as the next window approaches, during the window
    // it drains as time runs out. Hidden entirely between Cascade
    // cycles — but the cascade cycle is always running in this
    // prototype, so it's effectively always visible. The bar
    // still hides on game over (no point showing "Cascade
    // incoming" while the player is dead).
    spawn_bar(
        &mut commands,
        HUD_OFFSET_RIGHT + Vec2::new(0.0, CASCADE_BAR_OFFSET_Y),
        CASCADE_BAR_WIDTH,
        CASCADE_BAR_HEIGHT,
        CASCADE_BG,
        CASCADE_COLOR,
        HudElementKind::CascadeBar,
    );

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

    // ─── DIAGNOSTIC (feat/text2d-investigation) ──────────────────────────
    // Single boot-time log to confirm the text entities are spawning.
    // The actual layout/extract systems live in `bevy_text` and run in
    // PostUpdate regardless of our app code, so this is only useful
    // for confirming the entity was created with the right components.
    log::info!(
        "[text2d-probe] GameOver text entity spawned with TextFont font_size={} \
         and default Handle<Font> (will resolve to FiraMono only if `default_font` \
         feature is enabled; check Cargo.toml)",
        GAME_OVER_FONT_SIZE
    );
    // ─── END DIAGNOSTIC ──────────────────────────────────────────────────

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
        // Same explicit-Anchor + TextLayout as the game-over
        // text — see the comment there for the rationale.
        Anchor::Center,
        TextLayout::new_with_justify(JustifyText::Center),
        Transform::from_xyz(0.0, RESTART_PROMPT_OFFSET_Y, HUD_Z + 0.3),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::RestartPrompt,
        },
    ));
}

// Spawn a single full-screen overlay sprite that the cascade
// telegraph system tints each frame. Lives for the whole run
// (no despawn); the system flips its `Visibility` between
// Inherited and Hidden based on cascade state. Size is 2× the
// view dimensions so it always covers the screen even during
// the briefest camera-follow lag (when the player gets
// knocked back, the camera lerps and might catch up a frame
// late — a 2× overlay absorbs that without exposing any
// un-tinted border).
fn spawn_cascade_telegraph(mut commands: Commands) {
    commands.spawn((
        Sprite {
            color: CASCADE_TELEGRAPH_COLOR.with_alpha(0.0),
            custom_size: Some(Vec2::new(VIEW_WIDTH * 2.0, VIEW_HEIGHT * 2.0)),
            ..default()
        },
        // z above the HUD bars but below the game-over overlay
        // (which sits at HUD_Z + 0.2 sprite + HUD_Z + 0.3 text).
        // The cascade tint should be on the world, not on the
        // death screen, so HUD_Z + 0.4 puts it between them.
        Transform::from_xyz(0.0, 0.0, HUD_Z + 0.4),
        Visibility::Hidden,
        CascadeTelegraph,
    ));
}

// Update the cascade telegraph overlay each frame. Three
// states:
//
//   1. cascade.active == true → solid purple tint at
//      CASCADE_TELEGRAPH_ACTIVE_ALPHA. The world is "in
//      cascade mode" and the player is rewarded for noticing
//      (kills are 4×, etc, when those land in Phase 2).
//
//   2. cascade.active == false AND interval_timer.remaining_secs()
//      is in (0, CASCADE_TELEGRAPH_WARNING_LEAD_S] → purple
//      pulse, alpha oscillates between 0 and
//      CASCADE_TELEGRAPH_PULSE_MAX_ALPHA. The pulse gives
//      the player a 3s heads-up to either position for the
//      incoming slow-mo (push into a cluster) or back off
//      (if HP is low).
//
//   3. otherwise → hidden. Most of the run, the overlay is
//      invisible; the cascade bar in the HUD corner is the
//      only signal.
//
// Position: the overlay is anchored at the player's
// transform each frame so it always covers the camera
// viewport (which follows the player).
fn cascade_telegraph_update(
    cascade: Res<CascadeState>,
    player_query: Query<&Transform, (With<Player>, Without<CascadeTelegraph>)>,
    // `Without<HudElement>` is a no-op in practice (the overlay
    // never has `HudElement` — it's a different concept) but it
    // tells Bevy's disjointness checker that this query is
    // mutually exclusive with `hud_update_system`'s `hud_query`
    // (which is `With<HudElement>` and also needs `&mut
    // Transform`). Without the hint, Bevy raises B0001 because
    // both queries touch `Transform` and the checker can't
    // prove they don't overlap.
    mut overlay_query: Query<
        (&mut Transform, &mut Sprite, &mut Visibility),
        (With<CascadeTelegraph>, Without<HudElement>),
    >,
) {
    let Ok(player_tf) = player_query.single() else {
        return;
    };
    let Ok((mut tf, mut sprite, mut vis)) = overlay_query.single_mut() else {
        return;
    };

    // Position at the player each frame. Z is preserved so the
    // overlay doesn't fight the spawn-time z (HUD_Z + 0.4).
    tf.translation = Vec3::new(
        player_tf.translation.x,
        player_tf.translation.y,
        tf.translation.z,
    );

    if cascade.active {
        // State 1: active window. Solid tint, full alpha for the
        // active state.
        sprite.color = CASCADE_TELEGRAPH_COLOR.with_alpha(CASCADE_TELEGRAPH_ACTIVE_ALPHA);
        *vis = Visibility::Inherited;
    } else {
        // State 2 or 3: cooldown. Check whether we're inside the
        // warning window.
        let remaining = cascade.interval_timer.remaining_secs();
        if remaining > 0.0 && remaining <= CASCADE_TELEGRAPH_WARNING_LEAD_S {
            // State 2: warning pulse. Oscillate alpha between 0
            // and the pulse max. The pulse period is 0.5s (2 Hz)
            // so the player sees roughly 6 pulses during the 3s
            // window — enough to register as "something is
            // about to happen" without being annoying.
            // `elapsed_in_warning` is the time since the warning
            // started, used to phase the pulse.
            let elapsed_in_warning = CASCADE_TELEGRAPH_WARNING_LEAD_S - remaining;
            // |sin(2π × elapsed × freq)| gives a 0..1 oscillation.
            // freq = 2.0 Hz → period 0.5s → 6 full pulses in 3s.
            let pulse = (elapsed_in_warning * std::f32::consts::TAU * 2.0)
                .sin()
                .abs();
            let alpha = pulse * CASCADE_TELEGRAPH_PULSE_MAX_ALPHA;
            sprite.color = CASCADE_TELEGRAPH_COLOR.with_alpha(alpha);
            *vis = Visibility::Inherited;
        } else {
            // State 3: hidden. Far from the next window (either
            // just after one ended, or before the warning
            // window started).
            *vis = Visibility::Hidden;
        }
    }
}
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
    cascade: Res<CascadeState>,
    // `Option<&mut Sprite>` because the GameOver/RestartPrompt kinds
    // are split across TWO entities (a colored box and a text label,
    // see `spawn_hud`). The text entity has no `Sprite`; the box
    // does. The bar kinds have a sprite on every entity. The color-
    // update logic below is wrapped in `if let Some(...)` so the
    // text-only entity skips it cleanly.
    mut hud_query: Query<(
        &mut Transform,
        Option<&mut Sprite>,
        &mut Visibility,
        &HudElement,
    )>,
) {
    let Ok((player_tf, health, weapon)) = player_query.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();

    for (mut tf, sprite, mut vis, element) in &mut hud_query {
        match element.kind {
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
                        // Phase 1 (shotgun): color the fill by the current
                        // weapon so the player can tell at a glance which
                        // archetype they're holding. Background stays dark.
                        sprite.color = fire_rate_color(weapon.archetype);
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
                // Time bar wraps every TIME_BAR_WINDOW_S seconds so the
                // visual stays in a useful 0..1 range. The HUD never
                // shows the absolute elapsed time; players who want the
                // exact count can divide the bar position by window.
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
                // Cooldown: bar fills 0..1 as the next window
                // approaches (`interval_timer.elapsed_secs() /
                // interval_timer.duration().as_secs_f32()`).
                // Active: bar drains 1..0 as the window runs out
                // (`duration_timer.remaining_secs() /
                // duration_timer.duration().as_secs_f32()`).
                // Either reading produces a 0..1 ratio and the
                // choice between fill-up and drain-down gives
                // the player a clear "is the window open yet or
                // closing" signal at a glance.
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
