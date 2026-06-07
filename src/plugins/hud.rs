//! HUD: HP bar + fire-rate indicator.
//!
//! Day 13-14: minimal in-world HUD that stays screen-stable by being
//! re-positioned each frame at a fixed offset from the player (since
//! the camera follows the player, this is equivalent to a screen-
//! space HUD without the cost of a second camera + UI layer).
//!
//! Each element is a single Sprite with `Anchor::CenterLeft` so the
//! left edge is the transform's X. The fill amount is encoded in
//! `Transform::scale.x` — at scale 1.0 the bar is full-width, at
//! scale 0.0 the bar is invisible. Position is offset from the
//! player; size is fixed.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::components::{Health, HudElement, HudElementKind, Player, Weapon};
use crate::resources::GameSettings;

/// Color of the HP bar fill: lerps from green (full) to red (empty).
const HP_GREEN: Color = Color::srgb(0.30, 0.85, 0.30);
const HP_RED: Color = Color::srgb(0.85, 0.20, 0.20);
const HP_BG: Color = Color::srgb(0.10, 0.10, 0.10);

/// Color of the fire-rate indicator. Bright so it reads as "action
/// UI" against the dark health-bar background.
const FIRE_RATE_COLOR: Color = Color::srgb(0.95, 0.85, 0.30);
const FIRE_RATE_BG: Color = Color::srgb(0.10, 0.10, 0.10);

/// Width × height of the HP bar in world units (which == pixels at
/// 1:1 orthographic projection). The bar is 120 wide so it covers
/// ~10% of a 1280-wide window — readable but not dominant.
const HP_BAR_WIDTH: f32 = 120.0;
const HP_BAR_HEIGHT: f32 = 16.0;

/// Width × height of the fire-rate indicator. Smaller than the HP
/// bar so the eye reads HP as the primary stat.
const FIRE_RATE_WIDTH: f32 = 60.0;
const FIRE_RATE_HEIGHT: f32 = 8.0;

/// World-space offset of the HUD from the player. With the default
/// 1280×720 window, (player_x - 580, player_y + 320) lands the
/// HUD near the top-left of the screen.
const HUD_OFFSET: Vec2 = Vec2::new(-580.0, 320.0);
const FIRE_RATE_OFFSET_Y: f32 = -22.0;

/// Z value for HUD sprites — well above the player (0), enemies (0),
/// and projectiles (0.5), so the HUD always renders on top.
const HUD_Z: f32 = 10.0;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        // The HUD doesn't care about hitstop — it should keep updating
        // through the freeze so the player can see their HP drop.
        app.add_systems(Startup, spawn_hud)
            .add_systems(Update, hud_update_system);
    }
}

fn spawn_hud(mut commands: Commands) {
    // HP background — fixed size, dark. Always rendered behind the fill
    // (same Z, but spawned first so it appears first in render order).
    commands.spawn((
        Sprite {
            color: HP_BG,
            custom_size: Some(Vec2::new(HP_BAR_WIDTH, HP_BAR_HEIGHT)),
            anchor: Anchor::CenterLeft,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, HUD_Z),
        HudElement {
            kind: HudElementKind::HpBar,
        },
    ));

    // HP fill — colored by lerp(green, red, 1 - ratio). The update
    // system sets its scale.x to the current ratio and snaps the
    // color to the lerped value.
    commands.spawn((
        Sprite {
            color: HP_GREEN,
            custom_size: Some(Vec2::new(HP_BAR_WIDTH, HP_BAR_HEIGHT)),
            anchor: Anchor::CenterLeft,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, HUD_Z + 0.1),
        HudElement {
            kind: HudElementKind::HpBar,
        },
    ));

    // Fire-rate background + fill, same pattern as HP.
    commands.spawn((
        Sprite {
            color: FIRE_RATE_BG,
            custom_size: Some(Vec2::new(FIRE_RATE_WIDTH, FIRE_RATE_HEIGHT)),
            anchor: Anchor::CenterLeft,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, HUD_Z),
        HudElement {
            kind: HudElementKind::FireRate,
        },
    ));
    commands.spawn((
        Sprite {
            color: FIRE_RATE_COLOR,
            custom_size: Some(Vec2::new(FIRE_RATE_WIDTH, FIRE_RATE_HEIGHT)),
            anchor: Anchor::CenterLeft,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, HUD_Z + 0.1),
        HudElement {
            kind: HudElementKind::FireRate,
        },
    ));
}

fn hud_update_system(
    // `Without<HudElement>` is a no-op in practice (the Player never
    // carries one) but it tells Bevy's disjointness checker that this
    // query is mutually exclusive with the `hud_query` below, which
    // needs `&mut Transform`. Without the hint, Bevy raises B0001
    // because both queries touch `Transform`.
    player_query: Query<(&Transform, &Health, &Weapon), (With<Player>, Without<HudElement>)>,
    mut hud_query: Query<(&mut Transform, &mut Sprite, &HudElement)>,
) {
    let Ok((player_tf, health, weapon)) = player_query.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();

    // Each HudElement has both a background (BG_Z+0.0) and a fill
    // (BG_Z+0.1) variant, both tagged with the same HudElementKind.
    // We position both at the same anchor; the fill's scale.x and
    // color encode the current state.
    for (mut tf, mut sprite, element) in &mut hud_query {
        match element.kind {
            HudElementKind::HpBar => {
                let pos = player_pos + HUD_OFFSET;
                tf.translation = pos.extend(tf.translation.z);
                let ratio = (health.current / health.max).clamp(0.0, 1.0);
                tf.scale.x = ratio;
                // Lerp the fill color: green when full, red when empty.
                // The background sprite (z + 0.0) is unaffected by the
                // color since this is the same Sprite.color field; the
                // background is just a fixed dark rectangle at all times.
                // We discriminate by checking the current color's
                // luminance — backgrounds are HP_BG, fills aren't.
                if !approx_eq_color(sprite.color, HP_BG) {
                    sprite.color = lerp_color(HP_RED, HP_GREEN, ratio);
                }
            }
            HudElementKind::FireRate => {
                let pos = player_pos + HUD_OFFSET + Vec2::new(0.0, FIRE_RATE_OFFSET_Y);
                tf.translation = pos.extend(tf.translation.z);
                // Progress = 1 when the weapon is ready to fire (timer
                // remaining is 0), 0 just after firing.
                let interval = 1.0 / weapon.fire_rate;
                let remaining = weapon.fire_timer.remaining_secs();
                let progress = ((interval - remaining) / interval).clamp(0.0, 1.0);
                tf.scale.x = progress;
                if !approx_eq_color(sprite.color, FIRE_RATE_BG) {
                    sprite.color = FIRE_RATE_COLOR;
                }
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
