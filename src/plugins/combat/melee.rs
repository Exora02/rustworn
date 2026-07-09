//! Melee weapon system: click-to-swing arc attacks.
//!
//! On left-click while holding a `MeleeWeapon`, spawn a temporary hitbox
//! in front of the player. Damage enemies inside the hitbox and apply
//! knockback. A brief colored arc sprite provides visual feedback.

use bevy::prelude::*;

use crate::components::{
    AimDirection, Collider, Enemy, Health, Knockback, MeleeWeapon, Player, Team,
};
use crate::events::{DamageEvent, EnemyKilledEvent};
use crate::resources::{GameSettings, Hitstop, RunState};

use super::in_hitstop;

/// Color of the melee swing arc — warm orange-white, distinct from the
/// projectile's warm white and the beam's hot orange.
const MELEE_ARC_COLOR: Color = Color::srgb(1.0, 0.7, 0.2);

/// How long the arc visual lingers on screen (seconds).
const MELEE_ARC_VISUAL_DURATION_S: f32 = 0.1;

/// Default cooldown between melee swings (seconds).
const MELEE_COOLDOWN_S: f32 = 0.3;

/// Fired when the player swings their melee weapon. Used by audio system.
#[derive(Event)]
pub struct MeleeSwingEvent;

/// Marker component for the melee arc visual sprite.
#[derive(Component)]
pub struct MeleeArcVisual {
    pub timer: Timer,
}

/// On left-click while the player has a `MeleeWeapon`, spawn a temporary
/// hitbox in front of the player, damage enemies in it, and show the arc
/// visual. The hitbox is a small rectangle at `player_pos + aim_dir * range/2`.
#[allow(clippy::too_many_arguments)]
pub fn melee_swing_system(
    mouse: Res<ButtonInput<MouseButton>>,
    aim: Res<AimDirection>,
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    settings: Res<GameSettings>,
    run: Res<RunState>,
    mut commands: Commands,
    mut player_q: Query<(&Transform, &mut MeleeWeapon, Entity), With<Player>>,
    mut enemies: Query<
        (Entity, &Transform, &Collider, &mut Health),
        (With<Enemy>, Without<Player>),
    >,
    mut damage_events: EventWriter<DamageEvent>,
    mut kill_events: EventWriter<EnemyKilledEvent>,
    mut swing_events: EventWriter<MeleeSwingEvent>,
) {
    if in_hitstop(&hitstop) {
        return;
    }

    let Ok((player_tf, mut melee, player_entity)) = player_q.single_mut() else {
        return;
    };

    // Tick cooldown.
    melee.cooldown.tick(time.delta());
    if !melee.cooldown.finished() {
        return;
    }

    // Only swing on left-click press.
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let player_pos = player_tf.translation.truncate();
    let aim_dir = if aim.0 == Vec2::ZERO {
        Vec2::Y
    } else {
        aim.0.normalize()
    };

    let range = melee.range;
    let half_arc_rad = (melee.arc_deg / 2.0).to_radians();

    // Hitbox centre: halfway along the range.
    let hitbox_center = player_pos + aim_dir * (range / 2.0);

    // Hitbox half-extents: range in the aim direction, 30% of range
    // perpendicular. Clamp minimum hitbox size to avoid zero-area boxes.
    let hitbox_half_w = (range / 2.0).max(5.0);
    let hitbox_half_h = (range * 0.3).max(5.0);

    // Compute the damage multiplier from RunState.
    let damage_mult = 1.0 + run.damage_stacks as f32 * settings.pickup_damage_per_stack;
    let base_damage = melee.damage * damage_mult * run.base_damage;

    // Track which enemies we've hit this swing to avoid double-hit.
    let mut hit_count: u32 = 0;

    for (enemy_entity, enemy_tf, collider, mut health) in &mut enemies {
        let enemy_pos = enemy_tf.translation.truncate();

        // Check if the enemy is within the melee range.
        let to_enemy = enemy_pos - player_pos;
        let distance = to_enemy.length();
        if distance > range || distance < f32::EPSILON {
            continue;
        }

        // Check if the enemy is within the swing arc angle.
        let angle_to_enemy = to_enemy.angle_to(aim_dir).abs();
        if angle_to_enemy > half_arc_rad {
            continue;
        }

        // AABB overlap check: hitbox rectangle vs enemy collider.
        let to_hitbox_center = enemy_pos - hitbox_center;
        let fwd_proj = to_hitbox_center.dot(aim_dir);
        let perp_proj = to_hitbox_center.dot(Vec2::new(-aim_dir.y, aim_dir.x));

        let combined_half_w = hitbox_half_w + collider.half_extents.x;
        let combined_half_h = hitbox_half_h + collider.half_extents.y;

        if fwd_proj.abs() > combined_half_w || perp_proj.abs() > combined_half_h {
            continue;
        }

        // Apply damage.
        let final_damage = base_damage + (run.base_damage - 1.0) * base_damage;
        health.current = (health.current - final_damage).max(0.0);

        // Emit damage event.
        damage_events.write(DamageEvent {
            victim: enemy_entity,
            amount: final_damage,
        });

        // Apply knockback.
        let knockback_dir = to_enemy.normalize();
        commands.entity(enemy_entity).insert(Knockback {
            direction: knockback_dir * melee.knockback,
            timer: Timer::from_seconds(0.2, TimerMode::Once),
        });

        // Check for kill.
        if health.current <= 0.0 {
            kill_events.write(EnemyKilledEvent {
                victim: enemy_entity,
                killer: player_entity,
                position: enemy_pos,
            });
        }

        hit_count += 1;
    }

    // Emit swing event (for audio).
    swing_events.write(MeleeSwingEvent);

    // Spawn a temporary arc visual.
    let arc_pos = player_pos + aim_dir * (range * 0.6);

    commands.spawn((
        Sprite {
            color: MELEE_ARC_COLOR,
            custom_size: Some(Vec2::new(range * 1.2, range * 0.4)),
            ..default()
        },
        Transform::from_xyz(arc_pos.x, arc_pos.y, 0.55),
        MeleeArcVisual {
            timer: Timer::from_seconds(MELEE_ARC_VISUAL_DURATION_S, TimerMode::Once),
        },
    ));

    // Reset cooldown.
    let cooldown_s = MELEE_COOLDOWN_S / run.attack_speed_mult.max(0.1);
    melee.cooldown = Timer::from_seconds(cooldown_s, TimerMode::Once);

    log::info!(
        "Melee swing — hit {} enemies, damage={:.1}, range={:.1}",
        hit_count,
        base_damage,
        range,
    );
}

/// Despawn the arc visual sprites after their timer expires.
pub fn melee_visual_cleanup_system(
    time: Res<Time>,
    mut commands: Commands,
    mut visuals: Query<(Entity, &mut MeleeArcVisual)>,
) {
    for (entity, mut visual) in &mut visuals {
        visual.timer.tick(time.delta());
        if visual.timer.finished() {
            commands.entity(entity).despawn();
        }
    }
}