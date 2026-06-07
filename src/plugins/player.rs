//! Player movement, input, camera follow, aim, and dash.
//!
//! Day 1-2: WASD movement + smooth camera follow.
//! Day 3-4: mouse aim (white line indicator), Space-bar dash with i-frames.

use bevy::prelude::*;

use crate::components::{
    AimDirection, AimIndicator, BaseVelocity, CascadeAffected, Collider, DashCooldown, DeathPuff,
    Enemy, Health, Invulnerable, Knockback, MainCamera, Player, Projectile, Spawner, Velocity,
    Weapon, PLAYER_COLOR,
};
use crate::plugins::enemy::spawn_boss;
use crate::resources::{CascadeState, GameSettings, Hitstop, RunState, ScreenShake};
use log::info;

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
        // that early-return on it. time_tick / restart systems run
        // alongside (no chain dependency) so they can mutate RunState
        // independently of input/state/movement.
        //
        // cascade_state_system runs at the start of the chain so the
        // `active` flag is current by the time cascade-scaled
        // velocity_apply reads it.
        app.add_systems(
            Update,
            (
                hitstop_tick_system,
                cascade_state_system,
                time_tick_system,
                boss_spawn_system,
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
                restart_system,
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

// Day 15-16: returns true when the run is over (player HP reached
// zero). Used by every gameplay system to freeze uniformly so the
// game-over overlay and restart prompt can sit on a still frame.
// Visual systems (color flash, aim rotation, camera follow) still
// run — they need to keep rendering the dead player, frozen aim
// indicator, and a stable camera so the HUD stays screen-locked.
fn in_game_over(run: &RunState) -> bool {
    run.game_over
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

// Phase 1 cascade: drive the CascadeState cycle. While inactive,
// tick the `interval_timer` toward the next window. When it
// elapses, flip `active` to true and re-arm the `duration_timer`.
// While active, tick `duration_timer` and re-arm `interval_timer`
// when it elapses. We log the open/close so playtesters have a
// clear "did I just hit Cascade" signal in the console — visual
// telegraphing lands in Phase 2 with a screen tint + audio sting.
fn cascade_state_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut cascade: ResMut<CascadeState>,
) {
    if cascade.active {
        cascade.duration_timer.tick(time.delta());
        cascade.time_remaining = cascade.duration_timer.remaining_secs();
        if cascade.duration_timer.finished() {
            cascade.active = false;
            cascade.time_remaining = 0.0;
            cascade.kills_during = 0;
            cascade.chain_extends = 0;
            cascade.interval_timer =
                Timer::from_seconds(settings.cascade_interval_s, TimerMode::Once);
            log::info!("Cascade Mode ended — back to full speed.");
        }
    } else {
        cascade.interval_timer.tick(time.delta());
        if cascade.interval_timer.finished() {
            cascade.active = true;
            cascade.kills_during = 0;
            cascade.chain_extends = 0;
            cascade.duration_timer =
                Timer::from_seconds(settings.cascade_duration_s, TimerMode::Once);
            cascade.time_remaining = settings.cascade_duration_s;
            log::info!("Cascade Mode OPENED — 0.25× enemy speed for 30s.");
        }
    }
}

fn player_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut query: Query<&mut BaseVelocity, With<Player>>,
) {
    if in_hitstop(&hitstop) || in_game_over(&run) {
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
    run: Res<RunState>,
    mut commands: Commands,
    query: Query<Entity, ReadyPlayer>,
) {
    if in_hitstop(&hitstop) || in_game_over(&run) {
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
    run: Res<RunState>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut BaseVelocity, &mut Invulnerable), With<Player>>,
) {
    if in_hitstop(&hitstop) || in_game_over(&run) {
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
    run: Res<RunState>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut DashCooldown), With<Player>>,
) {
    if in_hitstop(&hitstop) || in_game_over(&run) {
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
    run: Res<RunState>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut BaseVelocity, &mut Knockback), With<Player>>,
) {
    if in_hitstop(&hitstop) || in_game_over(&run) {
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

// In Phase 1, the Cascade scaling lands here — every cascade-affected
// entity gets its Velocity multiplied by 0.25 each frame. Frozen during
// global hitstop so the cascade scaler doesn't overwrite the player/
// chaser velocities we're holding steady. Frozen during game over
// for the same reason.
//
// `Has<CascadeAffected>` lets the same query match both cascade-scaled
// and unscaled entities. Player + player projectiles don't carry the
// marker; enemies (and enemy projectiles, Phase 1+) do. The scale
// only applies on `CascadeState.active` — when inactive, even
// cascade-affected entities run at full speed.
fn velocity_apply_system(
    cascade: Res<CascadeState>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut query: Query<(&BaseVelocity, &mut Velocity, Has<CascadeAffected>)>,
) {
    if in_hitstop(&hitstop) || in_game_over(&run) {
        return;
    }
    // The scale lives in `GameSettings::cascade_scale` (default 0.25)
    // so a future "Cascade is a 0.5× slow-mo" variant can flip it
    // without touching this system.
    let scale = if cascade.active {
        settings.cascade_scale
    } else {
        1.0
    };
    for (base, mut vel, cascade_affected) in &mut query {
        vel.0 = if cascade_affected {
            base.0 * scale
        } else {
            base.0
        };
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
    run: Res<RunState>,
    mut query: Query<(&Velocity, &mut Transform)>,
) {
    if in_hitstop(&hitstop) || in_game_over(&run) {
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
// and flip RunState::game_over. All gameplay systems gate on
// `in_game_over(&run)` so the world freezes on a still frame, the
// HUD renders the game-over overlay, and the restart prompt sits
// waiting for the R key. `std::process::exit` was the previous
// behavior — landing a real game-over loop here means the binary
// can be re-run without a manual relaunch, and it's the foundation
// the proper Bevy State machine in Phase 2 will replace.
fn player_death_check_system(health: Query<&Health, With<Player>>, mut run: ResMut<RunState>) {
    if run.game_over {
        return;
    }
    let Ok(h) = health.single() else {
        return;
    };
    if h.current <= 0.0 {
        log::info!("RUSTWORN — you died. HP: 0.0");
        run.game_over = true;
    }
}

// Day 15-16: tick the per-run timer. Frozen during game over so the
// displayed time in the HUD reflects "time spent alive", not "time
// since the app launched". Reset by the restart system on R.
fn time_tick_system(time: Res<Time>, mut run: ResMut<RunState>) {
    if run.game_over {
        return;
    }
    run.time_elapsed += time.delta_secs();
}

// Phase 1 mini-boss: spawn the boss the frame `time_elapsed`
// crosses `boss_spawn_time_s` (60s default), exactly once per
// run. The spawn position is at a fixed distance from the
// player in a random direction (using the same xorshift
// pattern as `random_spawn_position` in the enemy plugin —
// duplicated here rather than promoted to a util to avoid
// pulling the spawn logic into the player plugin).
//
// The gate is `RunState.boss_spawned`, not "boss entity
// exists" — the entity is despawned on death and re-checked
// on restart. Using a separate flag decouples "boss has
// spawned this run" from "boss is currently alive" so the
// logic is clean (the entity existence check would also
// fire on a normal kill).
//
// Frozen during game over so a death at t=59.9s followed by
// no restart doesn't sneak a boss spawn in.
fn boss_spawn_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut run: ResMut<RunState>,
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
) {
    if run.game_over {
        return;
    }
    if run.boss_spawned {
        return;
    }
    if run.time_elapsed < settings.boss_spawn_time_s {
        return;
    }
    let Ok(player_tf) = player_query.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();

    // Cheap pseudo-random angle seeded from the system
    // clock. The boss spawns ONCE per run so "any angle is
    // fine" — a true RNG is overkill, and using a
    // constant direction would feel choreographed.
    let mut state = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xCAFEBABE);
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    let angle = ((state >> 40) as f32) / ((1u32 << 24) as f32)
        * std::f32::consts::TAU;
    let spawn_pos = player_pos
        + Vec2::new(angle.cos(), angle.sin()) * settings.boss_spawn_distance;

    spawn_boss(&mut commands, &settings, spawn_pos);
    run.boss_spawned = true;
    info!(
        "mini-boss spawned at t={:.1}s, pos=({:.1}, {:.1})",
        run.time_elapsed, spawn_pos.x, spawn_pos.y
    );
}

// Day 15-16: full-run restart. On R while game over, despawn every
// transient entity (enemies, projectiles, death puffs), reset the
// player's position/HP/weapon/dash-state, reset the spawner timer,
// reset the camera, zero out the screen shake, and clear the
// RunState counters. The intent is "the same world state as the
// moment the game booted" — no leftover chasers, no half-decayed
// puffs, no residual camera kick.
//
// `Without<MainCamera>` is implicit in the camera query below
// (camera already filters on MainCamera); the per-entity filters
// on the despawn queries are exclusive by component type, so the
// player, camera, HUD elements, and spawner are untouched.
fn restart_system(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<GameSettings>,
    mut run: ResMut<RunState>,
    mut player_query: Query<(Entity, &mut Transform, &mut Health, &mut Weapon), With<Player>>,
    mut spawner_query: Query<&mut Spawner>,
    mut camera_query: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
    mut shake: ResMut<ScreenShake>,
    mut cascade: ResMut<CascadeState>,
    mut commands: Commands,
    enemies: Query<Entity, With<Enemy>>,
    projectiles: Query<Entity, With<Projectile>>,
    puffs: Query<Entity, With<DeathPuff>>,
) {
    if !run.game_over {
        return;
    }
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }

    for e in &enemies {
        commands.entity(e).despawn();
    }
    for e in &projectiles {
        commands.entity(e).despawn();
    }
    for e in &puffs {
        commands.entity(e).despawn();
    }

    if let Ok((entity, mut tf, mut h, mut weapon)) = player_query.single_mut() {
        tf.translation = Vec3::ZERO;
        h.current = h.max;
        // Reset fire timer to a zero-duration finished timer so the
        // first click on the fresh run fires immediately (same as
        // the initial spawn in setup.rs).
        weapon.fire_timer = Timer::from_seconds(0.0, TimerMode::Once);
        // Phase 1 (burst): also reset the in-flight burst counter.
        // If the player died mid-burst (e.g. on round 2 of a 3-round
        // burst), the next run shouldn't inherit a half-spent
        // burst — the new run should boot with a fresh weapon
        // ready to fire on click.
        weapon.burst_rounds_remaining = 0;
        // Drop any leftover dash / hit / knockback state from the
        // previous run. The player boots clean, no i-frames, no
        // knockback direction, no cooldown.
        commands
            .entity(entity)
            .remove::<(Invulnerable, Knockback, DashCooldown, Collider)>();
        // Collider is re-added immediately so the fresh player is
        // collidable the instant gameplay resumes — removing and
        // re-adding in the same frame is a no-op visually but
        // ensures we get a fresh, default-sized collider.
        commands.entity(entity).insert(Collider {
            half_extents: Vec2::splat(20.0),
        });
    }

    for mut spawner in &mut spawner_query {
        spawner.interval = Timer::from_seconds(settings.spawner_interval_s, TimerMode::Repeating);
    }

    if let Ok(mut cam_tf) = camera_query.single_mut() {
        cam_tf.translation = Vec3::ZERO;
    }

    shake.intensity = 0.0;
    shake.duration = 0.0;
    shake.remaining = 0.0;

    // Phase 1 cascade: reset the cycle on restart so a fresh run
    // gets the standard warmup before the first Cascade window.
    // Without this, a Cascade that opened just before death would
    // persist (still active, but with no enemies to slow) into
    // the new run, and the player would see the HUD bar empty
    // immediately.
    cascade.active = false;
    cascade.time_remaining = 0.0;
    cascade.kills_during = 0;
    cascade.chain_extends = 0;
    cascade.interval_timer = Timer::from_seconds(30.0, TimerMode::Once);
    cascade.duration_timer = Timer::from_seconds(0.0, TimerMode::Once);

    // Phase 1 mini-boss: reset the boss spawn gate. The boss
    // itself is despawned via the `enemies` loop above
    // (carries the `Enemy` marker), but the gate flag
    // needs to flip back to false so the next run gets a
    // fresh boss spawn at the new `time_elapsed == 0 +
    // boss_spawn_time_s` crossing. Also reset the cascade
    // beacon to false — the flag flips to true on boss
    // death (the Phase 1 cascade hook) and the new run
    // starts without a beacon pending.
    run.boss_spawned = false;
    run.cascade_beacon_alive = false;

    run.game_over = false;
    run.time_elapsed = 0.0;
    run.enemies_killed = 0;
    run.score = 0;
}
