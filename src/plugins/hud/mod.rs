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

mod bars;
mod timer;
mod overlay;
pub mod juice;

use bevy::prelude::*;

use crate::components::AppState;

// ─── Layout offsets (top-left, top-right) ────────────────────────────────

/// Width of the player's view in world units. The HUD layout uses
/// this to position the top-left / top-right pairs at the screen
/// edges regardless of the window resolution.
pub(crate) const VIEW_WIDTH: f32 = 1280.0;
pub(crate) const VIEW_HEIGHT: f32 = 720.0;

pub(crate) const HUD_OFFSET: Vec2 = Vec2::new(-(VIEW_WIDTH * 0.5) + 60.0, VIEW_HEIGHT * 0.5 - 40.0);
pub(crate) const HUD_OFFSET_RIGHT: Vec2 = Vec2::new(
    VIEW_WIDTH * 0.5 - 60.0 - bars::KILL_BAR_WIDTH,
    VIEW_HEIGHT * 0.5 - 40.0,
);
pub(crate) const FIRE_RATE_OFFSET_Y: f32 = -22.0;
pub(crate) const TIME_BAR_OFFSET_Y: f32 = -16.0;

/// Z value for HUD sprites — well above the player (0), enemies (0),
/// and projectiles (0.5), so the HUD always renders on top.
pub(crate) const HUD_Z: f32 = 10.0;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        // Phase 2: the HUD updates through both InRun and GameOver
        // so the bars stay positioned and the game-over overlay
        // sits at the player's death position (the world is
        // frozen, but the camera + player don't move so the
        // bars' positions are stable). The HUD does NOT update
        // in MainMenu (no run to show progress for) — the
        // state_visibility_system hides all the bars there.
        app.add_systems(Startup, (spawn_hud, timer::spawn_cascade_telegraph))
            .add_systems(
                Update,
                (
                    // Cascade telegraph is gated to InRun only —
                    // it shouldn't pulse during GameOver (the
                    // world is frozen, no new Cascade window
                    // should open, the screen tint should hold
                    // steady on the death frame).
                    timer::cascade_telegraph_update.run_if(in_state(AppState::InRun)),
                    // HUD update runs in both InRun and GameOver.
                    // The `or` filter (instead of two separate
                    // `.add_systems` calls) keeps the existing
                    // chain ordering: cascade telegraph updates
                    // first so the overlay's z position is
                    // current before the HUD bars get re-sorted.
                    bars::hud_update_system
                        .run_if(in_state(AppState::InRun).or(in_state(AppState::GameOver))),
                    // Phase 4: pickup popup animation update. Runs
                    // during InRun only (no need to float popups
                    // while the world is frozen on death).
                    juice::pickup_popup_update
                        .run_if(in_state(AppState::InRun)),
                )
                    .chain(),
            );
    }
}

fn spawn_hud(mut commands: Commands) {
    bars::spawn_all_bars(&mut commands);
    overlay::spawn_game_over_overlay(&mut commands);
    juice::spawn_floor_indicator(&mut commands);
    juice::spawn_score_text(&mut commands);
}

// Linear color interpolation in sRGB. Both inputs are srgb; we
// extract the (r, g, b) channels, lerp, and reconstruct. Alpha is
// passed through (HUD elements are always opaque).
pub(crate) fn lerp_color(a: Color, b: Color, t: f32) -> Color {
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
pub(crate) fn approx_eq_color(c: Color, target: Color) -> bool {
    let c = c.to_srgba();
    let t = target.to_srgba();
    (c.red - t.red).abs() < 0.01
        && (c.green - t.green).abs() < 0.01
        && (c.blue - t.blue).abs() < 0.01
}
