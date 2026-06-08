//! Pickup system. Pickups are world-space entities that drift toward
//! the player over their lifetime and apply a permanent-for-the-run
//! effect on contact. The four kinds (Regen, DamageUp, FireRateUp,
//! SpeedUp) are defined in `components::PickupKind`.
//!
//! The drop happens at the kill site (in `combat.rs` / `enemy.rs`),
//! not here. This module owns the *lifecycle* of a pickup: drift,
//! auto-collect, despawn on lifetime expiry, and applying the
//! effect to the player on collect.

use bevy::prelude::*;

use crate::components::{AiBehavior, AppState, Health, Pickup, PickupKind, Player, Velocity};
use crate::resources::{GameSettings, RunState};

const PICKUP_SIZE: f32 = 14.0;

pub struct PickupsPlugin;

impl Plugin for PickupsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                pickup_drift_system,
                pickup_collect_system,
                pickup_lifetime_system,
                regen_tick_system,
            )
                .chain()
                // Phase 2: pickups freeze during MainMenu
                // (no run to lose them in) and GameOver (world
                // is frozen, pickups shouldn't drift off-screen
                // while the player reads the death screen).
                .run_if(in_state(AppState::InRun)),
        );
    }
}

/// Per-frame: drift each live pickup toward the player, and
/// despawn on lifetime expiry. Drift speed and lifetime are
/// on `GameSettings`. A pickup that touches the player is
/// collected (handled in `pickup_collect_system`); one that
/// expires is silently despawned (its drop was missed).
fn pickup_drift_system(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    player_q: Query<&Transform, With<Player>>,
    mut pickup_q: Query<(Entity, &mut Pickup, &mut Transform, &mut Velocity), Without<Player>>,
) {
    let Ok(player_tf) = player_q.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();
    let dt = time.delta_secs();
    for (entity, mut pickup, mut tf, mut vel) in &mut pickup_q {
        // Drift toward player only if outside collect radius.
        // Inside the radius, the collect system handles the
        // pickup this frame and we don't want to also pull
        // it inward.
        let to_player = player_pos - tf.translation.truncate();
        let dist = to_player.length();
        if dist > settings.pickup_collect_radius {
            let dir = if dist > 0.0 {
                to_player / dist
            } else {
                Vec2::ZERO
            };
            vel.0 = dir * settings.pickup_drift_speed;
        } else {
            vel.0 = Vec2::ZERO;
        }
        tf.translation += vel.0.extend(0.0) * dt;
        pickup.lifetime.tick(time.delta());
        if pickup.lifetime.finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Per-frame: collect any pickup whose center is inside the
/// collect radius. On collect: apply the effect, despawn the
/// pickup. Effects are stored on `RunState` so they persist
/// across the run; weapons read the multipliers in
/// `combat::weapon_fire_system` (for damage and fire rate) and
/// `player::player_movement` reads the speed multiplier.
fn pickup_collect_system(
    mut commands: Commands,
    settings: Res<GameSettings>,
    mut run: ResMut<RunState>,
    player_q: Query<&Transform, With<Player>>,
    pickup_q: Query<(Entity, &Pickup, &Transform)>,
    mut health_q: Query<&mut Health, With<Player>>,
) {
    let Ok(player_tf) = player_q.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();
    for (entity, pickup, tf) in &pickup_q {
        let pos = tf.translation.truncate();
        if pos.distance(player_pos) > settings.pickup_collect_radius {
            continue;
        }
        apply_pickup(pickup.kind, &mut run, &settings, &mut health_q);
        commands.entity(entity).despawn();
    }
}

/// Apply the effect of a picked-up `PickupKind` to the run
/// state. The damage/fire-rate/speed multipliers stack
/// multiplicatively (1.0 + n × per_stack), so 2 DamageUp =
/// 1.5625× damage. Regen is special-cased: it doesn't stack
/// a multiplier, it stacks a flat HP/sec regen. Each Regen
/// pickup adds another +0.5 HP/sec to the running total.
fn apply_pickup(
    kind: PickupKind,
    run: &mut RunState,
    settings: &GameSettings,
    health_q: &mut Query<&mut Health, With<Player>>,
) {
    match kind {
        PickupKind::DamageUp => {
            run.damage_stacks = (run.damage_stacks + 1).min(8);
        }
        PickupKind::FireRateUp => {
            run.fire_rate_stacks = (run.fire_rate_stacks + 1).min(8);
        }
        PickupKind::SpeedUp => {
            run.speed_stacks = (run.speed_stacks + 1).min(8);
        }
        PickupKind::Regen => {
            run.regen_per_sec += settings.pickup_regen_hp_per_sec;
        }
    }
    // Heal a small chunk on every pickup so the player can
    // feel the collect even when the stacked effect is small.
    if let Ok(mut h) = health_q.single_mut() {
        h.current = (h.current + 1.0).min(h.max);
    }
}

/// Per-frame regen: if `RunState.regen_per_sec > 0`, add
/// `regen_per_sec * dt` to the player's HP, capped at max.
/// Capped stacks on the multipliers (max 8 per kind) keep
/// the run from snowballing; regen has no cap because the
/// HP cap is the natural ceiling.
pub fn regen_tick_system(
    time: Res<Time>,
    run: Res<RunState>,
    mut health_q: Query<&mut Health, With<Player>>,
) {
    if run.regen_per_sec <= 0.0 {
        return;
    }
    let dt = time.delta_secs();
    if let Ok(mut h) = health_q.single_mut() {
        h.current = (h.current + run.regen_per_sec * dt).min(h.max);
    }
}

/// Convenience: roll whether a kill of `behavior` should drop a
/// pickup. Returns `Some(PickupKind)` on a successful roll, `None`
/// otherwise. The drop chance is per-archetype on `GameSettings`.
/// The `rng` parameter is a closure that returns a `f32` in [0, 1)
/// for testability — production calls a deterministic hash of
/// the kill counter (Phase 2 will swap in seeded RNG).
pub fn roll_pickup_drop(
    behavior: AiBehavior,
    settings: &GameSettings,
    rng: &mut impl FnMut() -> f32,
) -> Option<PickupKind> {
    let chance = match behavior {
        AiBehavior::Chaser => settings.pickup_drop_chance_chaser,
        AiBehavior::Dasher => settings.pickup_drop_chance_dasher,
        AiBehavior::Shooter => settings.pickup_drop_chance_shooter,
        AiBehavior::Sniper => settings.pickup_drop_chance_sniper,
        AiBehavior::Boss => settings.pickup_drop_chance_boss,
    };
    if rng() >= chance {
        return None;
    }
    // Equal-weight roll for the kind. Phase 1 has 4 kinds;
    // Phase 2 will weight these (more DamageUp / Regen
    // than FireRateUp / SpeedUp, since the weapons feel
    // less differentiated and stat boosts feel more
    // impactful). The shape is "1 of N" — `rng` × 4
    // truncated.
    let r = (rng() * 4.0) as u32;
    Some(match r {
        0 => PickupKind::DamageUp,
        1 => PickupKind::FireRateUp,
        2 => PickupKind::SpeedUp,
        _ => PickupKind::Regen,
    })
}

/// Lifetime: separate from the per-frame drift + tick, so the
/// drift system can focus on motion and the lifetime is a
/// background tick. (Both are checked each frame in the same
/// pass; the split is logical, not physical.) Exposed as a
/// system label in case Phase 2 wants to insert pickup visual
/// effects (glow, audio sting) on the despawn frame.
fn pickup_lifetime_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Pickup)>,
) {
    for (entity, mut pickup) in &mut q {
        pickup.lifetime.tick(time.delta());
        if pickup.lifetime.finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Spawn a pickup of `kind` at `pos`. The pickup is a small
/// world-space sprite that drifts toward the player over its
/// lifetime. The visual is a colored square (one color per
/// kind) so the player can read the kind at a glance — no
/// text needed.
pub fn spawn_pickup(commands: &mut Commands, settings: &GameSettings, pos: Vec2, kind: PickupKind) {
    let color = match kind {
        PickupKind::Regen => Color::srgb(0.30, 0.95, 0.30),
        PickupKind::DamageUp => Color::srgb(0.95, 0.30, 0.30),
        PickupKind::FireRateUp => Color::srgb(0.95, 0.95, 0.30),
        PickupKind::SpeedUp => Color::srgb(0.30, 0.65, 0.95),
    };
    commands.spawn((
        Pickup {
            kind,
            lifetime: Timer::from_seconds(settings.pickup_lifetime_s, TimerMode::Once),
        },
        Sprite {
            color,
            custom_size: Some(Vec2::splat(PICKUP_SIZE)),
            ..default()
        },
        Transform::from_xyz(pos.x, pos.y, 0.3),
        Velocity(Vec2::ZERO),
        Visibility::Inherited,
    ));
}
