//! Cascade telegraph overlay — full-screen tint during Cascade windows.
//!
//! A single full-screen sprite that tints the world during a
//! Cascade window. The visual progression:
//!
//!   idle     — overlay hidden, world is normal
//!   warning  — 3s before the next window opens, purple
//!              pulses at ~2 Hz, alpha 0..MAX_PULSE
//!   active   — window is open, solid purple tint at ACTIVE_ALPHA
//!
//! All three states use the same sprite entity; only the
//! color/alpha/visibility change frame to frame.

use bevy::prelude::*;

use crate::components::{CascadeTelegraph, HudElement, Player};
use crate::resources::CascadeState;

use super::HUD_Z;

/// Tint color for the cascade overlay — matches the cascade bar
/// in the HUD so the player sees the same purple both in the
/// corner indicator and across the whole world.
pub const CASCADE_TELEGRAPH_COLOR: Color = Color::srgb(0.45, 0.20, 0.85);

/// Alpha during the active window. Low enough that the world is
/// still readable (you need to see enemies through the tint) but
/// high enough that the cascade state is unmistakable.
pub const CASCADE_TELEGRAPH_ACTIVE_ALPHA: f32 = 0.30;

/// Peak alpha during the 3s warning pulse. Lower than the active
/// alpha so the warning reads as a "preparing to happen" signal
/// rather than the cascade itself being live.
pub const CASCADE_TELEGRAPH_PULSE_MAX_ALPHA: f32 = 0.18;

/// How many seconds before the cascade window opens the warning
/// pulse begins. 3s gives the player enough time to register
/// the visual change, prepare positioning, and plan a route
/// into the densest enemy cluster.
pub const CASCADE_TELEGRAPH_WARNING_LEAD_S: f32 = 3.0;

// Spawn a single full-screen overlay sprite that the cascade
// telegraph system tints each frame. Lives for the whole run
// (no despawn); the system flips its `Visibility` between
// Inherited and Hidden based on cascade state. Size is 2× the
// view dimensions so it always covers the screen even during
// the briefest camera-follow lag (when the player gets
// knocked back, the camera lerps and might catch up a frame
// late — a 2× overlay absorbs that without exposing any
// un-tinted border).
pub fn spawn_cascade_telegraph(mut commands: Commands) {
    commands.spawn((
        Sprite {
            color: CASCADE_TELEGRAPH_COLOR.with_alpha(0.0),
            custom_size: Some(Vec2::new(super::VIEW_WIDTH * 2.0, super::VIEW_HEIGHT * 2.0)),
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
pub fn cascade_telegraph_update(
    cascade: Res<CascadeState>,
    player_query: Query<&Transform, (With<Player>, Without<CascadeTelegraph>)>,
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

    tf.translation = Vec3::new(
        player_tf.translation.x,
        player_tf.translation.y,
        tf.translation.z,
    );

    if cascade.active {
        sprite.color = CASCADE_TELEGRAPH_COLOR.with_alpha(CASCADE_TELEGRAPH_ACTIVE_ALPHA);
        *vis = Visibility::Inherited;
    } else {
        let remaining = cascade.interval_timer.remaining_secs();
        if remaining > 0.0 && remaining <= CASCADE_TELEGRAPH_WARNING_LEAD_S {
            let elapsed_in_warning = CASCADE_TELEGRAPH_WARNING_LEAD_S - remaining;
            let pulse = (elapsed_in_warning * std::f32::consts::TAU * 2.0)
                .sin()
                .abs();
            let alpha = pulse * CASCADE_TELEGRAPH_PULSE_MAX_ALPHA;
            sprite.color = CASCADE_TELEGRAPH_COLOR.with_alpha(alpha);
            *vis = Visibility::Inherited;
        } else {
            *vis = Visibility::Hidden;
        }
    }
}
