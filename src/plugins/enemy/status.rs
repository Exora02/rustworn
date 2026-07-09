//! Status effects system — Burn, Slow, Stun tick logic.
//!
//! `status_effect_tick_system` runs every frame for enemies with
//! `StatusEffects`. Each active timer is ticked; expired timers are
//! cleared. The system also modifies `BaseVelocity` for Slow/Stun.
//!
//! Burn: deals 0.5 damage/s while active (3s duration = 1.5 total dmg).
//! Slow: halves `BaseVelocity` while active (2s duration).
//! Stun: zeros `BaseVelocity` while active (1s duration — no
//!   movement or attacks).

use bevy::prelude::*;

use crate::components::{BaseVelocity, Health, StatusEffects};

/// Tick all active status effects on enemies.
///
/// Ordering expectations:
/// - Runs AFTER `enemy_ai_system` (which writes `BaseVelocity`) so Slow/Stun
///   can override the AI-set velocity.
/// - Runs BEFORE `velocity_apply_system` so the modified `BaseVelocity`
///   flows into `Velocity` for the physics integration step.
/// - Burn damage per second is applied as `0.5 * dt` each frame, NOT
///   front-loaded, so the tick is smooth.
pub fn status_effect_tick_system(
    time: Res<Time>,
    mut commands: Commands,
    mut enemies: Query<(Entity, &mut StatusEffects, Option<&mut BaseVelocity>, Option<&mut Health>)>,
) {
    let dt = time.delta();
    let burn_dps = 0.5;

    for (entity, mut effects, mut base_vel, mut health) in &mut enemies {
        // ── Burn ──────────────────────────────────────────────────────
        if let Some(timer) = &mut effects.burn_timer {
            timer.tick(dt);
            // Apply burn damage every frame (0.5 dps * dt).
            if let Some(ref mut hp) = health {
                hp.current = (hp.current - burn_dps * dt.as_secs_f32()).max(0.0);
            }
            if timer.just_finished() {
                effects.burn_timer = None;
            }
        }

        // ── Slow ──────────────────────────────────────────────────────
        if let Some(timer) = &mut effects.slow_timer {
            timer.tick(dt);
            if timer.just_finished() {
                effects.slow_timer = None;
            }
        }

        // ── Stun ──────────────────────────────────────────────────────
        if let Some(timer) = &mut effects.stun_timer {
            timer.tick(dt);
            if timer.just_finished() {
                effects.stun_timer = None;
            }
        }

        // ── Apply velocity modifications ──────────────────────────────
        // Apply AFTER effect clearing so expired effects don't linger
        // on the velocity for one frame.
        if let Some(ref mut vel) = base_vel {
            if effects.stun_timer.is_some() {
                // Stun: no movement at all.
                vel.0 = Vec2::ZERO;
            } else if effects.slow_timer.is_some() {
                // Slow: half speed.
                vel.0 *= 0.5;
            }
        }

        // ── Remove component if all effects are empty ─────────────────
        if effects.burn_timer.is_none()
            && effects.slow_timer.is_none()
            && effects.stun_timer.is_none()
        {
            commands.entity(entity).remove::<StatusEffects>();
        }
    }
}

/// Roll whether a status effect applies.
/// Returns `true` if `rng.next()` < `chance`.
fn roll_status(rng: &mut impl FnMut() -> f32, chance: f32) -> bool {
    rng() < chance
}

/// Apply status effects to an enemy entity. Called from projectile/beam
/// damage systems on every hit. Each effect has an independent roll;
/// if any succeed, a single `StatusEffects` component is inserted
/// (or existing one is overwritten with the new timers).
///
/// Roll chances:
/// - Burn: 10% chance, 3s duration
/// - Slow: 15% chance, 2s duration
/// - Stun: 5% chance, 1s duration
pub fn apply_status_effects(
    commands: &mut Commands,
    entity: Entity,
    rng: &mut impl FnMut() -> f32,
) {
    let burn_timer = if roll_status(rng, 0.10) {
        Some(Timer::from_seconds(3.0, TimerMode::Once))
    } else {
        None
    };
    let slow_timer = if roll_status(rng, 0.15) {
        Some(Timer::from_seconds(2.0, TimerMode::Once))
    } else {
        None
    };
    let stun_timer = if roll_status(rng, 0.05) {
        Some(Timer::from_seconds(1.0, TimerMode::Once))
    } else {
        None
    };

    if burn_timer.is_some() || slow_timer.is_some() || stun_timer.is_some() {
        commands.entity(entity).insert(StatusEffects {
            burn_timer,
            slow_timer,
            stun_timer,
        });
    }
}