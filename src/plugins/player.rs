//! Player movement, input, camera follow, aim, and dash.
//!
//! Day 1-2: WASD movement + smooth camera follow.
//! Day 3-4: mouse aim (white line indicator), Space-bar dash with i-frames.

use bevy::prelude::*;

use crate::components::{
    AimDirection, AimIndicator, AppState, BaseVelocity, Boss, CascadeAffected, DashCooldown,
    Health, Invulnerable, Knockback, LastMoveDirection, MainCamera, Player, RoomKind, Velocity,
    PLAYER_COLOR,
};
use crate::plugins::enemy::{spawn_beacon, spawn_boss};
use crate::resources::{CascadeState, FloorState, GameSettings, Hitstop, RunState, ScreenShake};
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
        // that early-return on it. time_tick / boss_spawn systems run
        // alongside (no chain dependency) so they can mutate RunState
        // independently of input/state/movement.
        //
        // cascade_state_system runs at the start of the chain so the
        // `active` flag is current by the time cascade-scaled
        // velocity_apply reads it.
        //
        // Phase 2: the whole chain is gated on `in_state(AppState::InRun)`.
        // This replaces the Phase 1 `if in_game_over(&run) { return; }`
        // early-return at the top of every system. The chain runs
        // during a live run and pauses (frozen world, all
        // `Movement` / `AI` / `Weapons` systems idle) during
        // MainMenu and GameOver. The `restart_system` that used to
        // be the last system in this chain has been moved to
        // `enter_in_run_system` in setup.rs (it now fires on
        // `OnEnter(AppState::InRun)`).
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
                // Phase 2: `movement_system` removed. Its replacement,
                // `physics_step_system` in rooms.rs, integrates
                // Velocity into Transform AND resolves wall
                // collisions. The player chain ends at
                // velocity_apply (which writes Velocity from
                // BaseVelocity + Cascade scale); the rooms chain
                // picks up from there with the actual Transform
                // integration.
                player_color_system,
                aim_rotation_system,
                camera_follow_system,
                player_death_check_system,
            )
                .chain()
                .run_if(in_state(AppState::InRun)),
        );
    }
}

// Helper: returns true when a global hitstop is active. Used by
// gameplay systems (movement, AI, weapon fire) to freeze uniformly.
fn in_hitstop(hitstop: &Option<Res<Hitstop>>) -> bool {
    hitstop.is_some()
}

// Day 15-16 (removed in Phase 2): the previous `in_game_over` helper
// gated gameplay systems on `RunState.game_over: bool`. Phase 2 lifts
// the bool into a Bevy `AppState` and gates the whole player chain
// on `in_state(AppState::InRun)` at the chain level. The per-system
// `if in_hitstop(&hitstop) || in_game_over(&run) { return; }` lines
// are now just `if in_hitstop(&hitstop) { return; }` — the run_if on
// the chain handles the game-over freeze uniformly.

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
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
) {
    if cascade.active {
        cascade.duration_timer.tick(time.delta());
        cascade.time_remaining = cascade.duration_timer.remaining_secs();
        if cascade.duration_timer.finished() {
            cascade.active = false;
            cascade.time_remaining = 0.0;
            let _kills = cascade.kills_during;
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

            // Spawn a beacon near the player so they can
            // chain-extend by killing it during the window.
            if let Ok(player_tf) = player_query.single() {
                let player_pos = player_tf.translation.truncate();
                // Cheap pseudo-random angle from RunRng is
                // ideal but the RunRng is seeded from system
                // time at run-entry; the beacon spawns on a
                // cascade-open, not run-entry, so we use the
                // same clock-hash heuristic as boss_spawn_system.
                let mut state = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0xCAFEBABE);
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let angle = ((state >> 40) as f32) / ((1u32 << 24) as f32)
                    * std::f32::consts::TAU;
                let dist = settings.beacon_spawn_distance_min
                    + ((state >> 16) as f32) / ((1u32 << 24) as f32)
                        * (settings.beacon_spawn_distance_max - settings.beacon_spawn_distance_min);
                let pos = player_pos + Vec2::new(angle.cos(), angle.sin()) * dist;
                spawn_beacon(&mut commands, &settings, pos);
                log::info!("Cascade beacon spawned at ({:.1}, {:.1})", pos.x, pos.y);
            }
        }
    }
}

fn player_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut query: Query<(&mut BaseVelocity, &mut LastMoveDirection), With<Player>>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    let Ok((mut base_velocity, mut last_move)) = query.single_mut() else {
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
    // Phase 1 pickup multiplier: each SpeedUp pickup adds
    // `pickup_speed_per_stack` (0.20) to a 1.0 base. 2 SpeedUp
    // = 1.0 × 1.2 × 1.2 = 1.44× move speed.
    let speed_mult = 1.0 + run.speed_stacks as f32 * settings.pickup_speed_per_stack;
    let move_dir = if dir.length() > 0.0 {
        dir.normalize()
    } else {
        Vec2::ZERO
    };
    base_velocity.0 = if move_dir != Vec2::ZERO {
        move_dir * settings.player_move_speed * speed_mult
    } else {
        Vec2::ZERO
    };
    // Record the last non-zero movement direction so the dash
    // system knows which way the player was going. Only update
    // on input — while standing still, the previous direction
    // is preserved (a dash with no WASD goes the way you were
    // last moving, not the way the mouse is pointing).
    if move_dir != Vec2::ZERO {
        last_move.0 = move_dir;
    }
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
    _run: Res<RunState>,
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
// `last_move_dir × dash_speed` — this overrides the input
// system for the duration of the iframe. The dash direction
// is the WASD movement direction (where the player is
// going), not the aim direction (where the mouse is). The
// aim direction is used only as a first-dash fallback: if
// the player has never pressed WASD since spawn, the dash
// still has a sensible direction (their last-known aim, or
// straight up if neither has been recorded).
fn dash_state_system(
    time: Res<Time>,
    aim: Res<AimDirection>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    _run: Res<RunState>,
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &mut BaseVelocity,
            &mut Invulnerable,
            &LastMoveDirection,
        ),
        With<Player>,
    >,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    for (entity, mut base, mut inv, last_move) in &mut query {
        inv.0.tick(time.delta());
        if inv.0.finished() {
            base.0 = Vec2::ZERO;
            commands.entity(entity).remove::<Invulnerable>();
            let cd = Timer::from_seconds(settings.player_dash_cooldown_s, TimerMode::Once);
            commands.entity(entity).insert(DashCooldown(cd));
        } else {
            // Prefer the last WASD movement direction. Fall
            // back to aim if the player hasn't moved yet
            // (`LastMoveDirection` defaults to `Vec2::ZERO`
            // before the first input), and finally straight
            // up if neither has been set.
            let dir = if last_move.0 != Vec2::ZERO {
                last_move.0
            } else if aim.0 != Vec2::ZERO {
                aim.0
            } else {
                Vec2::Y
            };
            base.0 = dir * settings.player_dash_speed;
        }
    }
}

fn cooldown_tick_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    _run: Res<RunState>,
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
    _run: Res<RunState>,
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
    _run: Res<RunState>,
    mut query: Query<(&BaseVelocity, &mut Velocity, Has<CascadeAffected>)>,
) {
    if in_hitstop(&hitstop) {
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

// Phase 2: `movement_system` (which lived here through Phase 0
// and Phase 1) has been moved to `rooms.rs` as part of
// `physics_step_system`. The new system combines velocity
// integration + wall collision resolution in a single
// system to avoid B0001 across two same-Transform-writing
// queries. The "no integration during hitstop" behavior is
// preserved (the new system early-returns on hitstop).
// Projectiles are still integrated; they just skip wall
// collision in Patch 2 (a future patch can despawn
// projectiles on wall hit).

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
// and transition to `AppState::GameOver` via `NextState`. The whole
// player chain is gated on `in_state(AppState::InRun)`, so the frame
// we transition, every gameplay system (movement, AI, fire, contact
// damage) early-returns and the world freezes on a still frame. The
// `state_visibility_system` in setup.rs flips the game-over overlay
// to `Visibility::Inherited` so the player sees "GAME OVER" + the
// R-restart prompt.
//
// Phase 1 used `run.game_over = true` here and gated every system
// on that bool. Phase 2 lifts the bool into a proper `States` enum
// so the title screen + pause state can land without another
// refactor.
fn player_death_check_system(
    health: Query<&Health, With<Player>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let Ok(h) = health.single() else {
        return;
    };
    if h.current <= 0.0 {
        log::info!("RUSTWORN — you died. HP: 0.0");
        next_state.set(AppState::GameOver);
    }
}

// Day 15-16: tick the per-run timer. The chain is gated on
// `in_state(AppState::InRun)`, so this only runs during a live run
// — no game-over early-return needed. The displayed time in the HUD
// reflects "time spent alive", not "time since the app launched".
// Reset by `enter_in_run_system` on each InRun entry.
fn time_tick_system(time: Res<Time>, mut run: ResMut<RunState>) {
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
// no restart doesn't sneak a boss spawn in. (The chain is gated
// on InRun, so this is automatic — but the comment stays for
// future readers tracing the timing.)
fn boss_spawn_system(
    _time: Res<Time>,
    settings: Res<GameSettings>,
    mut run: ResMut<RunState>,
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    floor: Res<FloorState>,
    bosses: Query<(), With<Boss>>,
) {
    // Per-room boss spawning: each boss room gets its own boss instance.
    // Check if a Boss entity already exists in the world (for any room).
    // If we're in a Boss room and no boss is alive, spawn one immediately.
    let in_boss_room = floor
        .rooms
        .iter()
        .any(|r| r.id == floor.current_room && r.kind == RoomKind::Boss);
    let boss_alive = bosses.iter().next().is_some();

    if in_boss_room && !boss_alive {
        let Ok(player_tf) = player_query.single() else { return; };
        let _player_pos = player_tf.translation.truncate();
        // Spawn at the room center, offset upward slightly so it's visible.
        let Some(room) = floor.rooms.iter().find(|r| r.id == floor.current_room) else { return; };
        let room_center = (room.bounds_min + room.bounds_max) * 0.5;
        let spawn_pos = room_center + Vec2::new(0.0, 60.0);
        let clamped = spawn_pos.clamp(
            room.bounds_min + Vec2::splat(60.0),
            room.bounds_max - Vec2::splat(60.0),
        );
        spawn_boss(&mut commands, &settings, &floor, clamped);
        info!(
            "Boss spawned at boss room {}, pos=({:.1}, {:.1})",
            floor.current_room.0, clamped.x, clamped.y
        );
        return;
    }

    // Timer-based fallback (Phase 1 legacy): spawn at run midpoint if
    // player hasn't reached a boss room yet.
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
    let angle = ((state >> 40) as f32) / ((1u32 << 24) as f32) * std::f32::consts::TAU;
    let spawn_pos = player_pos + Vec2::new(angle.cos(), angle.sin()) * settings.boss_spawn_distance;

    // Clamp the boss spawn position inside the current room.
    // Otherwise the boss can end up outside walls where the
    // player can't reach it.
    let clamped = floor
        .rooms
        .iter()
        .find(|r| r.id == floor.current_room)
        .map(|room| {
            let margin = 60.0;
            spawn_pos.clamp(
                room.bounds_min + Vec2::splat(margin),
                room.bounds_max - Vec2::splat(margin),
            )
        })
        .unwrap_or(spawn_pos);

    spawn_boss(&mut commands, &settings, &floor, clamped);
    run.boss_spawned = true;
    info!(
        "mini-boss spawned at t={:.1}s, pos=({:.1}, {:.1})",
        run.time_elapsed, spawn_pos.x, spawn_pos.y
    );
}

// Phase 2: the Phase 1 `restart_system` (Day 15-16) has been
// replaced by `enter_in_run_system` in setup.rs. The new system
// fires on every `OnEnter(AppState::InRun)` — both the first run
// (after MainMenu) and every subsequent restart (after GameOver).
// The body is identical to the old `restart_system` minus the
// key check + the `run.game_over` gate, and it lives in
// setup.rs because the lifecycle is "start of a run", not
// "player input".
