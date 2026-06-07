//! Player movement, input, camera follow, aim, and dash.
//!
//! Day 1-2: WASD movement + smooth camera follow.
//! Day 3-4: mouse aim (white line indicator), Space-bar dash with i-frames.

use bevy::prelude::*;

use crate::components::{
    AimDirection, AimIndicator, BaseVelocity, DashCooldown, Health, Invulnerable, Knockback,
    MainCamera, Player, Velocity, PLAYER_COLOR,
};
use crate::resources::{GameSettings, Hitstop, ScreenShake};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        // AimDirection is a Resource the player systems read every
        // frame; it must be explicitly initialized, or every
        // Res<AimDirection> / ResMut<AimDirection> access panics.
        app.init_resource::<AimDirection>();

        // Order matters: input → state overrides → velocity copy →
        // movement + visuals → camera follow. hitstop_tick runs first
        // so its resource-mutation takes effect before the systems
        // that early-return on it.
        app.add_systems(
            Update,
            (
                hitstop_tick_system,
                player_input_system,
                aim_input_system,
                dash_input_system,
                dash_state_system,
                cooldown_tick_system,
                knockback_system,
                velocity_apply_system,
                movement_system,
                player_color_system,
                aim_rotation_system,
                camera_follow_system,
                player_death_check_system,
            )
                .chain(),
        );
    }
}

// Helper: returns true when a global hitstop is active. Used by
// gameplay systems (movement, AI, weapon fire) to freeze uniformly.
fn in_hitstop(hitstop: &Option<Res<Hitstop>>) -> bool {
    hitstop.is_some()
}

// Day 13-14: tick the global Hitstop resource. When the timer
// elapses, remove the resource so the `Option<Res<Hitstop>>` checks
// in every gameplay system return None and the freeze lifts.
//
// `Option<ResMut<Hitstop>>` is required (not just `ResMut`) because
// the resource is only inserted on a killing blow — every other
// frame it's absent, and `ResMut` would panic on the missing
// resource. The default Bevy error handler escalates a missing
// resource to a panic, which would prevent the game from ever
// reaching the kill that creates the resource in the first place.
fn hitstop_tick_system(time: Res<Time>, mut commands: Commands, hitstop: Option<ResMut<Hitstop>>) {
    let Some(mut hitstop) = hitstop else {
        return;
    };
    hitstop.timer.tick(time.delta());
    if hitstop.timer.finished() {
        commands.remove_resource::<Hitstop>();
    }
}

fn player_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    mut query: Query<&mut BaseVelocity, With<Player>>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    let Ok(mut base_velocity) = query.single_mut() else {
        return;
    };

    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        dir.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        dir.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        dir.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        dir.x += 1.0;
    }

    // Diagonal is the same speed as cardinal — normalize if there's input.
    base_velocity.0 = if dir.length() > 0.0 {
        dir.normalize() * settings.player_move_speed
    } else {
        Vec2::ZERO
    };
}

// Tracks the mouse's world-space direction from the player. Sticks at
// the previous value when the cursor is dead-on the player (so dashing
// still has a direction to use).
fn aim_input_system(
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    player_query: Query<&Transform, With<Player>>,
    mut aim: ResMut<AimDirection>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, cam_transform)) = camera_query.single() else {
        return;
    };
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(cam_transform, cursor_pos) else {
        return;
    };

    let dir = world_pos - player_transform.translation.truncate();
    // Keep last known direction if cursor is on the player (avoid jitter
    // from floating-point wobble near the origin).
    if dir.length_squared() > 1.0 {
        aim.0 = dir.normalize();
    }
}

/// Player query used by systems that need a single player entity
/// and want to filter by dash state (no iframe, no cooldown).
type ReadyPlayer = (With<Player>, Without<Invulnerable>, Without<DashCooldown>);

// Space-bar → start a dash. Requires neither Invulnerable nor DashCooldown
// to be present. The dash direction is the current aim; if aim is
// uninitialized (e.g. game just booted, mouse never moved), default up.
fn dash_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<GameSettings>,
    aim: Res<AimDirection>,
    hitstop: Option<Res<Hitstop>>,
    mut commands: Commands,
    query: Query<Entity, ReadyPlayer>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    if !keys.just_pressed(KeyCode::Space) {
        return;
    }
    let Ok(player) = query.single() else {
        return;
    };

    let duration = Timer::from_seconds(settings.player_dash_duration_s, TimerMode::Once);
    commands.entity(player).insert(Invulnerable(duration));

    // Default dash direction if the player hasn't aimed yet.
    let _ = aim.0; // AimDirection is consumed in dash_state_system below.
}

// While Invulnerable, the player's BaseVelocity is forced to
// `aim_dir × dash_speed` — this overrides the input system for the
// duration of the iframe. When the timer elapses, the Invulnerable
// component is removed and a DashCooldown is attached.
fn dash_state_system(
    time: Res<Time>,
    aim: Res<AimDirection>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut BaseVelocity, &mut Invulnerable), With<Player>>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    for (entity, mut base, mut inv) in &mut query {
        inv.0.tick(time.delta());
        if inv.0.finished() {
            base.0 = Vec2::ZERO;
            commands.entity(entity).remove::<Invulnerable>();
            let cd = Timer::from_seconds(settings.player_dash_cooldown_s, TimerMode::Once);
            commands.entity(entity).insert(DashCooldown(cd));
        } else {
            // Use the last known aim direction; fall back to +Y so the
            // first dash after game boot has a sensible direction.
            let dir = if aim.0 == Vec2::ZERO { Vec2::Y } else { aim.0 };
            base.0 = dir * settings.player_dash_speed;
        }
    }
}

fn cooldown_tick_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut DashCooldown), With<Player>>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    for (entity, mut cd) in &mut query {
        cd.0.tick(time.delta());
        if cd.0.finished() {
            commands.entity(entity).remove::<DashCooldown>();
        }
    }
}

// While Knockback is present, the player's BaseVelocity is forced to
// the knockback direction at the configured speed. When the timer
// elapses, the component is removed and normal input resumes. Runs
// after `player_input_system` (so the input write is overridden) and
// before `dash_state_system` (so a fresh dash wins over a stale
// knockback — useful as an escape).
fn knockback_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut BaseVelocity, &mut Knockback), With<Player>>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    for (entity, mut base, mut kb) in &mut query {
        kb.timer.tick(time.delta());
        if kb.timer.finished() {
            commands.entity(entity).remove::<Knockback>();
        } else {
            base.0 = kb.direction * settings.chaser_knockback_speed;
        }
    }
}

// In Phase 1, the Cascade scaling will land here — every cascade-affected
// entity gets its Velocity multiplied by 0.25 each frame. Frozen during
// global hitstop so the cascade scaler doesn't overwrite the player/
// chaser velocities we're holding steady.
fn velocity_apply_system(
    hitstop: Option<Res<Hitstop>>,
    mut query: Query<(&BaseVelocity, &mut Velocity)>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    for (base, mut vel) in &mut query {
        vel.0 = base.0;
    }
}

// Applies `Velocity` to `Transform` for every entity that has both.
// Used by the player and by projectiles (Day 5-7+). The Cascade-aware
// velocity scaling will live in `velocity_apply_system` (Phase 1).
// Frozen during global hitstop so neither the player nor projectiles
// drift during the freeze.
fn movement_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    mut query: Query<(&Velocity, &mut Transform)>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    for (velocity, mut transform) in &mut query {
        transform.translation.x += velocity.0.x * time.delta_secs();
        transform.translation.y += velocity.0.y * time.delta_secs();
    }
}

// Player color: orange by default, pulse-toward-white during the dash
// iframe window. The reset on invuln-end is automatic because the system
// runs every frame and snaps back to PLAYER_COLOR when Invulnerable is gone.
fn player_color_system(
    time: Res<Time>,
    mut query: Query<(&mut Sprite, Has<Invulnerable>), With<Player>>,
) {
    for (mut sprite, invulnerable) in &mut query {
        if invulnerable {
            // 60Hz pulse, 70% max mix toward white — at the default
            // 0.15s dash this gives ~9 frames of visible flash.
            let t = (time.elapsed_secs() * 60.0).sin() * 0.5 + 0.5;
            let flash = t * 0.7;
            sprite.color = Color::srgb(
                0.925 + (1.0 - 0.925) * flash,
                0.525 + (1.0 - 0.525) * flash,
                0.012 + (1.0 - 0.012) * flash,
            );
        } else {
            sprite.color = PLAYER_COLOR;
        }
    }
}

// Rotates the aim indicator child to point in the current aim direction.
// The sprite is laid out along its local +Y axis, so we subtract PI/2
// from the world-space aim angle to convert "atan2(y,x) points right"
// into "rotation that turns local +Y toward the aim direction".
fn aim_rotation_system(
    aim: Res<AimDirection>,
    mut query: Query<&mut Transform, With<AimIndicator>>,
) {
    if aim.0 == Vec2::ZERO {
        return;
    }
    let angle = aim.0.y.atan2(aim.0.x) - std::f32::consts::FRAC_PI_2;
    for mut transform in &mut query {
        transform.rotation = Quat::from_rotation_z(angle);
    }
}

// Frame-rate independent exponential smoothing. The camera approaches
// the player at `lerp_speed` units per second. On top of the smooth
// follow, a random offset is added whenever `ScreenShake.remaining >
// 0` — the offset magnitude decays linearly over the shake window,
// giving the camera a "kick" effect on player damage.
fn camera_follow_system(
    time: Res<Time>,
    player_query: Query<&Transform, With<Player>>,
    mut camera_query: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
    mut shake: ResMut<ScreenShake>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let Ok(mut camera_transform) = camera_query.single_mut() else {
        return;
    };

    let lerp_speed = 8.0;
    let t = 1.0 - (-lerp_speed * time.delta_secs()).exp();
    let target = camera_transform
        .translation
        .lerp(player_transform.translation, t);

    // Day 13-14: screen shake. Decay remaining, then add a random
    // offset whose magnitude scales with (remaining / duration). The
    // seed for randomness is the current time so the shake isn't
    // identical between frames (which would be a static jitter).
    //
    // Apply the offset on top of the lerped target (=), NOT on top of
    // the previous frame's camera position (+=). Accumulating the
    // offset across frames would create a slow drift and also leak
    // a residual offset into the post-shake frames, which would
    // visibly misalign the camera from the player for a beat.
    let mut final_pos = target;
    if shake.remaining > 0.0 {
        shake.remaining = (shake.remaining - time.delta_secs()).max(0.0);
        let magnitude = shake.intensity * (shake.remaining / shake.duration);
        let now = time.elapsed_secs();
        final_pos += Vec2::new(
            (now * 173.7).sin() * magnitude,
            (now * 251.3).cos() * magnitude,
        )
        .extend(0.0);
    }
    camera_transform.translation = final_pos;
}

// Day 8-10: when the player's health hits zero, log a death message
// and exit. The real game-over screen lands in Phase 2.
fn player_death_check_system(health: Query<&Health, With<Player>>) {
    let Ok(h) = health.single() else {
        return;
    };
    if h.current <= 0.0 {
        log::info!("RUSTWORN — you died. HP: 0.0");
        std::process::exit(0);
    }
}
