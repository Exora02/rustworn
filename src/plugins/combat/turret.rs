//! Engineer deployable turret — auto-targeting sentry that fires at
//! the nearest enemy. Deployed with KeyE, recalled with KeyE.
//! Despawns automatically after a lifetime expires.

use bevy::prelude::*;

use crate::components::{
    BaseVelocity, CascadeAffected, Collider, Enemy, Player, PlayerClass, Projectile, Team, Velocity,
};
use crate::resources::{GameSettings, Hitstop, RunState};

/// Turret sprite size — small blue square.
const TURRET_SIZE: f32 = 20.0;
/// Turret color — bright blue so it reads as "friendly deployable".
const TURRET_COLOR: Color = Color::srgb(0.2, 0.5, 1.0);
/// Projectile color — lighter blue to distinguish from player projectiles.
const TURRET_PROJECTILE_COLOR: Color = Color::srgb(0.4, 0.7, 1.0);
/// Projectile size — small bullet.
const TURRET_PROJECTILE_SIZE: Vec2 = Vec2::new(4.0, 8.0);
/// Turret projectile speed (units/sec).
const TURRET_PROJECTILE_SPEED: f32 = 400.0;
/// Turret fire interval (seconds).
const TURRET_FIRE_INTERVAL_S: f32 = 0.5;
/// Turret lifetime (seconds).
const TURRET_LIFETIME_S: f32 = 15.0;
/// Turret cooldown on deploy/recall (seconds).
const TURRET_COOLDOWN_S: f32 = 4.0;

/// Marker component for a deployed engineer turret entity.
#[derive(Component)]
pub struct Turret {
    /// Timer between auto-fires.
    pub fire_timer: Timer,
    /// Total lifetime before the turret despawns.
    pub lifetime: Timer,
}

/// Spawn or recall the engineer's turret on KeyE press.
/// - If no turret is deployed: deploy one at the player's position.
/// - If a turret IS deployed: despawn it (recall).
/// Both cost the same cooldown.
pub fn engineer_turret_deploy_system(
    keys: Res<ButtonInput<KeyCode>>,
    hitstop: Option<Res<Hitstop>>,
    mut run: ResMut<RunState>,
    mut commands: Commands,
    player_query: Query<(Entity, &Transform), With<Player>>,
    turret_query: Query<Entity, With<Turret>>,
) {
    if hitstop.is_some() {
        return;
    }
    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }
    // Only Engineer class can use this ability.
    if run.selected_class != PlayerClass::Engineer {
        return;
    }
    // Respect cooldown.
    if !run.engineer_turret_cooldown.finished() {
        return;
    }
    let Ok((_player_entity, player_tf)) = player_query.single() else {
        return;
    };

    // If a turret is already deployed, recall it.
    if let Some(turret_entity) = turret_query.iter().next() {
        commands.entity(turret_entity).despawn();
        run.engineer_turret_deployed = false;
        run.engineer_turret_cooldown =
            Timer::from_seconds(TURRET_COOLDOWN_S, TimerMode::Once);
        return;
    }

    // Deploy a new turret at the player's position.
    let player_pos = player_tf.translation.truncate();
    let pos = Vec3::new(player_pos.x, player_pos.y, 0.5);

    commands.spawn((
        Turret {
            fire_timer: Timer::from_seconds(TURRET_FIRE_INTERVAL_S, TimerMode::Once),
            lifetime: Timer::from_seconds(TURRET_LIFETIME_S, TimerMode::Once),
        },
        Sprite {
            color: TURRET_COLOR,
            custom_size: Some(Vec2::splat(TURRET_SIZE)),
            ..default()
        },
        Transform::from_translation(pos),
        Collider {
            half_extents: Vec2::splat(TURRET_SIZE * 0.5),
        },
    ));
    run.engineer_turret_deployed = true;
    run.engineer_turret_cooldown =
        Timer::from_seconds(TURRET_COOLDOWN_S, TimerMode::Once);
}

/// For each deployed turret, find the nearest enemy and fire a
/// projectile toward it.
pub fn turret_auto_fire_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut commands: Commands,
    mut turret_query: Query<(&Transform, &mut Turret)>,
    enemy_query: Query<&Transform, With<Enemy>>,
) {
    if hitstop.is_some() {
        return;
    }

    for (turret_tf, mut turret) in &mut turret_query {
        turret.fire_timer.tick(time.delta());
        if !turret.fire_timer.finished() {
            continue;
        }

        // Find the nearest enemy.
        let turret_pos = turret_tf.translation.truncate();

        let mut nearest_distance = f32::MAX;
        let mut nearest_pos = Vec2::ZERO;

        for enemy_tf in &enemy_query {
            let enemy_pos = enemy_tf.translation.truncate();
            let dist = turret_pos.distance_squared(enemy_pos);
            if dist < nearest_distance {
                nearest_distance = dist;
                nearest_pos = enemy_pos;
            }
        }

        if nearest_distance == f32::MAX {
            // No enemies — skip this fire cycle but reset timer so we
            // check again after the normal interval.
            turret.fire_timer =
                Timer::from_seconds(TURRET_FIRE_INTERVAL_S, TimerMode::Once);
            continue;
        }

        // Spawn a projectile toward the enemy.
        let dir = (nearest_pos - turret_pos).normalize_or_zero();
        let spawn_pos = turret_pos + dir * (TURRET_SIZE * 0.5 + TURRET_PROJECTILE_SIZE.y * 0.5 + 2.0);
        let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;

        let damage = 1.0 + run.damage_stacks as f32 * 0.2;

        commands.spawn((
            Sprite {
                color: TURRET_PROJECTILE_COLOR,
                custom_size: Some(TURRET_PROJECTILE_SIZE),
                ..default()
            },
            Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5)
                .with_rotation(Quat::from_rotation_z(angle)),
            BaseVelocity(dir * TURRET_PROJECTILE_SPEED),
            Velocity(dir * TURRET_PROJECTILE_SPEED),
            CascadeAffected,
            Collider {
                half_extents: Vec2::new(
                    TURRET_PROJECTILE_SIZE.x * 0.5,
                    TURRET_PROJECTILE_SIZE.y * 0.5,
                ),
            },
            Projectile {
                damage,
                lifetime: Timer::from_seconds(settings.projectile_lifetime_s, TimerMode::Once),
                team: Team::Player,
            },
        ));

        turret.fire_timer =
            Timer::from_seconds(TURRET_FIRE_INTERVAL_S, TimerMode::Once);
    }
}

/// Tick each turret's lifetime timer. When expired, despawn the
/// turret and reset the engineer_turret_deployed flag.
pub fn turret_lifetime_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    mut run: ResMut<RunState>,
    mut commands: Commands,
    mut turret_query: Query<(Entity, &mut Turret)>,
) {
    if hitstop.is_some() {
        return;
    }

    for (entity, mut turret) in &mut turret_query {
        turret.lifetime.tick(time.delta());
        if turret.lifetime.finished() {
            commands.entity(entity).despawn();
            run.engineer_turret_deployed = false;
        }
    }
}

/// Tick the engineer turret cooldown each frame.
pub fn engineer_turret_cooldown_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    mut run: ResMut<RunState>,
) {
    if hitstop.is_some() {
        return;
    }
    run.engineer_turret_cooldown.tick(time.delta());
}