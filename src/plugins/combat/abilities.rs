//! Character ability systems: grenade throw (Marauder), teleport (Spectre),
//! dash-strike (Reaver), turret (Engineer).
//!
//! # Marauder — Grenade Throw
//!
//! On `KeyE` when `PlayerClass::Marauder`, spawn a grenade projectile that
//! travels in the aim direction. The grenade explodes on enemy contact or
//! after 1.5s, damaging all enemies within an 80px radius.
//!
//! Cooldown: 3s. Tracked on `RunState::marauder_ability_cooldown`.

use bevy::prelude::*;
use log::warn;

use crate::components::{
    AimDirection, Collider, Enemy, Grenade, Health, HitFlash, Player, PlayerClass,
};
use crate::resources::{GameSettings, Hitstop, RunState};

use super::in_hitstop;

/// Color of the Marauder grenade — bright orange so it's clearly
/// distinguishable from regular projectiles.
const GRENADE_COLOR: Color = Color::srgb(1.0, 0.55, 0.0);
/// Grenade sprite size: a chunky circle (square sprite, visually a circle).
const GRENADE_SIZE: Vec2 = Vec2::splat(16.0);
/// Grenade collider: 8px half-extents (matching 16x16 sprite).
const GRENADE_HALF_EXTENTS: Vec2 = Vec2::splat(8.0);
/// Default aim direction when the player hasn't aimed yet.
const DEFAULT_AIM: Vec2 = Vec2::Y;
/// Grenade travel speed (pixels/second).
const GRENADE_SPEED: f32 = 350.0;
/// Grenade lifetime before auto-explosion (seconds).
const GRENADE_LIFETIME_S: f32 = 1.5;
/// Grenade explosion radius (pixels).
const GRENADE_RANGE: f32 = 80.0;
/// Marauder ability cooldown (seconds).
const MARAUDER_COOLDOWN_S: f32 = 3.0;
/// Base damage of the grenade explosion.
const GRENADE_BASE_DAMAGE: f32 = 3.0;

/// Spawn a grenade projectile when the Marauder presses E.
///
/// System flow:
/// 1. Check that the player is Marauder, has pressed E, is not in hitstop,
///    and the ability cooldown is finished.
/// 2. Read the aim direction and spawn a grenade entity.
/// 3. Set the cooldown timer to 3s.
///
/// The grenade travels under its own `marauder_grenade_movement_system`
/// until it either hits an enemy (explodes immediately) or its 1.5s
/// lifetime expires (auto-explosion).
#[allow(clippy::too_many_arguments)]
pub fn marauder_grenade_system(
    keys: Res<ButtonInput<KeyCode>>,
    aim: Res<AimDirection>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    mut run: ResMut<RunState>,
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
) {
    if in_hitstop(&hitstop) {
        return;
    }

    // Only Marauder class, E key just pressed.
    if run.selected_class != PlayerClass::Marauder {
        return;
    }
    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }

    // Cooldown check.
    if !run.marauder_ability_cooldown.finished() {
        warn!("Marauder grenade on cooldown");
        return;
    }

    let Ok(player_tf) = player_query.single() else {
        return;
    };

    let dir = if aim.0 == Vec2::ZERO {
        DEFAULT_AIM
    } else {
        aim.0
    };

    let player_pos = player_tf.translation.truncate();
    let spawn_pos = player_pos + dir * settings.weapon_muzzle_offset;

    // Scale damage with damage_stacks: 1 + damage_stacks * 0.2
    let damage_mult = 1.0 + run.damage_stacks as f32 * settings.pickup_damage_per_stack;
    let damage = GRENADE_BASE_DAMAGE * damage_mult;

    // Spawn the grenade projectile.
    commands.spawn((
        Sprite {
            color: GRENADE_COLOR,
            custom_size: Some(GRENADE_SIZE),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5),
        Collider {
            half_extents: GRENADE_HALF_EXTENTS,
        },
        Grenade {
            timer: Timer::from_seconds(GRENADE_LIFETIME_S, TimerMode::Once),
            range: GRENADE_RANGE,
            damage,
            direction: dir,
            speed: GRENADE_SPEED,
        },
    ));

    // Start cooldown.
    run.marauder_ability_cooldown =
        Timer::from_seconds(MARAUDER_COOLDOWN_S, TimerMode::Once);

    warn!(
        "Marauder threw grenade — direction ({:.2}, {:.2}), damage {:.1}",
        dir.x, dir.y, damage
    );
}

/// Advance each grenade's lifetime and move it along its direction.
/// When the timer expires, trigger the explosion.
pub fn marauder_grenade_movement_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    mut commands: Commands,
    mut grenades: Query<(Entity, &mut Grenade, &mut Transform)>,
) {
    if in_hitstop(&hitstop) {
        return;
    }

    for (entity, mut grenade, mut transform) in &mut grenades {
        grenade.timer.tick(time.delta());

        // Move the grenade.
        transform.translation += (grenade.direction * grenade.speed * time.delta_secs()).extend(0.0);

        // If the timer expired, trigger explosion.
        if grenade.timer.finished() {
            commands.entity(entity).despawn();
            // The explosion damage is resolved by the damage system below.
            // We use a marker component approach: we insert Exploding marker
            // so the explosion system can process it; but since we already
            // despawned the entity, we write a GrenadeExplosionEvent instead.
            // Actually, let's handle it right here by spawning a temporary
            // explosion marker that the damage system picks up.
            // Simpler: write a small event for the explosion.
            commands.trigger(GrenadeExplosionEvent {
                position: transform.translation.truncate(),
                range: grenade.range,
                damage: grenade.damage,
            });
        }
    }
}

/// Event fired when a grenade explodes (by lifetime expiry or enemy contact).
/// The explosion system reads this and deals damage to all enemies within range.
#[derive(Event)]
pub struct GrenadeExplosionEvent {
    pub position: Vec2,
    pub range: f32,
    pub damage: f32,
}

/// Resolve grenade explosions: read `GrenadeExplosionEvent` events, check all
/// enemies within `range` of the explosion position, and apply damage.
#[allow(clippy::too_many_arguments)]
pub fn marauder_grenade_explosion_system(
    settings: Res<GameSettings>,
    mut run: ResMut<RunState>,
    mut commands: Commands,
    mut explosions: EventReader<GrenadeExplosionEvent>,
    mut enemies: Query<(Entity, &Transform, &Collider, &mut Health)>,
    // We use a separate query for mutation below
) {
    for explosion in explosions.read() {
        warn!(
            "Grenade explosion at ({:.1}, {:.1}) — range {:.0}, damage {:.1}",
            explosion.position.x, explosion.position.y, explosion.range, explosion.damage
        );

        // Collect enemies within range.
        for (enemy_entity, enemy_tf, _enemy_col, mut health) in enemies.iter_mut() {
            let enemy_pos = enemy_tf.translation.truncate();

            // Distance check from explosion center to enemy center.
            let dist = (enemy_pos - explosion.position).length();
            if dist > explosion.range {
                continue;
            }

            // Apply damage.
            health.current = (health.current - explosion.damage).max(0.0);

            // Visual feedback.
            commands.entity(enemy_entity).insert(HitFlash {
                timer: Timer::from_seconds(settings.hit_flash_duration_s, TimerMode::Once),
            });

            warn!(
                "Grenade hit enemy at ({:.1}, {:.1}) — HP now {:.1}",
                enemy_pos.x, enemy_pos.y, health.current
            );

            // If the enemy died, despawn it.
            if health.current <= 0.0 {
                commands.entity(enemy_entity).despawn();
                run.enemies_killed = run.enemies_killed.saturating_add(1);
            }
        }
    }
}

/// Cooldown tick system: advance the marauder ability cooldown timer.
/// Runs every frame so the cooldown progresses regardless of player input.
pub fn ability_cooldown_tick_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    mut run: ResMut<RunState>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    // Marauder cooldown
    run.marauder_ability_cooldown.tick(time.delta());
}

/// Check grenade-vs-enemy AABB overlap. When a grenade overlaps an enemy,
/// explode it immediately (despawn the grenade + fire explosion event).
pub fn marauder_grenade_impact_system(
    mut commands: Commands,
    grenades: Query<(Entity, &Transform, &Grenade)>,
    enemies: Query<(&Transform, &Collider), With<Enemy>>,
) {
    for (grenade_entity, grenade_tf, grenade) in &grenades {
        let grenade_pos = grenade_tf.translation.truncate();

        for (enemy_tf, enemy_col) in &enemies {
            let enemy_pos = enemy_tf.translation.truncate();
            let delta = (grenade_pos - enemy_pos).abs();
            if delta.x < GRENADE_HALF_EXTENTS.x + enemy_col.half_extents.x
                && delta.y < GRENADE_HALF_EXTENTS.y + enemy_col.half_extents.y
            {
                // Grenade hit an enemy — trigger explosion.
                commands.entity(grenade_entity).despawn();
                commands.trigger(GrenadeExplosionEvent {
                    position: grenade_pos,
                    range: grenade.range,
                    damage: grenade.damage,
                });
                break; // One explosion per grenade.
            }
        }
    }
}