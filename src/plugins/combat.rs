//! Combat: weapons, projectiles, and damage.
//!
//! Day 5-7: click-to-fire pistol, projectile with lifetime, no enemies yet.
//! Damage routing (Day 11-12) will land here too.

use bevy::prelude::*;

use crate::components::{
    AimDirection, BaseVelocity, Collider, Enemy, Health, HitFlash, Player, Projectile, Team,
    Velocity, Weapon,
};
use crate::plugins::enemy::spawn_death_puff;
use crate::resources::{GameSettings, Hitstop};

/// Color of the player projectile — warm white, clearly readable as a
/// bullet against any background we'll add later.
const PROJECTILE_COLOR: Color = Color::srgb(1.0, 0.95, 0.6);

/// Sprite size of the projectile. Long axis aligns with travel direction.
const PROJECTILE_SIZE: Vec2 = Vec2::new(6.0, 12.0);

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        // Order: cooldown tick (drives `fire_timer.finished()`) → fire
        // (consumes click + spawns projectile) → projectile lifetime
        // (despawns old ones) → projectile-damage (resolves hits
        // against enemies). Movement is handled by the generic
        // `movement_system` in the player plugin.
        app.add_systems(
            Update,
            (
                weapon_cooldown_tick_system,
                weapon_fire_system,
                projectile_lifetime_system,
                projectile_damage_system,
            )
                .chain(),
        );
    }
}

// Day 13-14: helper to early-return when a global hitstop is active.
// Used by every combat system that ticks timers, mutates transforms,
// or fires projectiles — the freeze is felt uniformly because all of
// them gate on this. The resource is removed by `hitstop_tick_system`
// in the player plugin once its timer elapses.
fn in_hitstop(hitstop: &Option<Res<Hitstop>>) -> bool {
    hitstop.is_some()
}

// Each frame, advance every weapon's fire_timer. A freshly fired weapon
// has its timer set to `1.0 / fire_rate`; once it ticks down to zero the
// weapon can fire again. Frozen during global hitstop so the player
// can't circumvent the freeze by firing.
fn weapon_cooldown_tick_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    mut query: Query<&mut Weapon>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    for mut weapon in &mut query {
        weapon.fire_timer.tick(time.delta());
    }
}

// Left-click on a ready weapon → spawn a Projectile traveling in the
// current aim direction. The projectile gets a `Velocity` but no
// `BaseVelocity` (player projectiles don't cascade-scale). Frozen
// during global hitstop.
fn weapon_fire_system(
    mouse: Res<ButtonInput<MouseButton>>,
    aim: Res<AimDirection>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    mut commands: Commands,
    mut weapons: Query<(&mut Weapon, &Transform), With<Player>>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok((mut weapon, player_transform)) = weapons.single_mut() else {
        return;
    };
    if !weapon.fire_timer.finished() {
        return;
    }

    // Reset cooldown. The combat plugin's tick system will count it down
    // each frame; the next fire becomes possible once the timer elapses.
    let interval = 1.0 / weapon.fire_rate;
    weapon.fire_timer = Timer::from_seconds(interval, TimerMode::Once);

    // Default to +Y if the player hasn't aimed yet.
    let dir = if aim.0 == Vec2::ZERO { Vec2::Y } else { aim.0 };

    // Spawn at the muzzle (player center + aim × offset) and rotate the
    // long axis of the sprite to match the aim direction.
    let spawn_pos = player_transform.translation.truncate() + dir * settings.weapon_muzzle_offset;
    let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;

    commands.spawn((
        Sprite {
            color: PROJECTILE_COLOR,
            custom_size: Some(PROJECTILE_SIZE),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5)
            .with_rotation(Quat::from_rotation_z(angle)),
        // No `BaseVelocity` / `CascadeAffected` markers, so the shared
        // `velocity_apply_system` won't touch this entity and the spawn-
        // time Velocity is preserved as the projectile's true motion.
        // Player projectiles run at full speed during Cascade Mode
        // (per PLAN.md §2).
        Velocity(dir * weapon.projectile_speed),
        // Day 11-12: 6x12 sprite → 3x6 half-extents for AABB collision
        // against enemy hitboxes. Kept slightly smaller than the visual
        // so close calls feel like hits rather than near-misses.
        Collider {
            half_extents: Vec2::new(PROJECTILE_SIZE.x * 0.5, PROJECTILE_SIZE.y * 0.5),
        },
        Projectile {
            damage: weapon.damage,
            lifetime: Timer::from_seconds(settings.projectile_lifetime_s, TimerMode::Once),
            team: Team::Player,
        },
    ));
}

fn projectile_lifetime_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Projectile)>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    for (entity, mut proj) in &mut query {
        proj.lifetime.tick(time.delta());
        if proj.lifetime.finished() {
            commands.entity(entity).despawn();
        }
    }
}

// AABB-vs-AABB overlap test between two transformed entities.
// Same math as the player contact-damage system, but duplicated here
// rather than promoted to a shared util — there's only two callers
// and they live in different plugins.
fn aabb_overlap(pos_a: Vec2, half_a: Vec2, pos_b: Vec2, half_b: Vec2) -> bool {
    let delta = (pos_a - pos_b).abs();
    delta.x < half_a.x + half_b.x && delta.y < half_a.y + half_b.y
}

// Iterate live projectiles and check each against every chaser. On
// hit: despawn the projectile, decrement the chaser's health, and
// (if HP hits zero) despawn the chaser and spawn a fading DeathPuff
// at its last position. One projectile = at most one enemy hit; the
// projectile despawns on first contact, so the inner loop exits
// naturally.
fn projectile_damage_system(
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    mut commands: Commands,
    projectiles: Query<(Entity, &Transform, &Collider, &Projectile)>,
    mut enemies: Query<(Entity, &Transform, &Collider, &mut Health), With<Enemy>>,
) {
    // Frozen during global hitstop — a killing blow that triggers the
    // freeze should not also resolve follow-up hits in the same frame.
    if in_hitstop(&hitstop) {
        return;
    }
    for (proj_entity, proj_tf, proj_col, proj) in &projectiles {
        // Projectiles are Player team today; the team check is in
        // place so enemy projectiles (Phase 1+) can co-exist without
        // damaging their own faction.
        if !matches!(proj.team, Team::Player) {
            continue;
        }
        let proj_pos = proj_tf.translation.truncate();

        let mut hit = false;
        for (enemy_entity, enemy_tf, enemy_col, mut health) in &mut enemies {
            if !aabb_overlap(
                proj_pos,
                proj_col.half_extents,
                enemy_tf.translation.truncate(),
                enemy_col.half_extents,
            ) {
                continue;
            }
            health.current = (health.current - proj.damage).max(0.0);
            hit = true;
            // Day 13-14: white pulse on every hit (kill or not).
            commands.entity(enemy_entity).insert(HitFlash {
                timer: Timer::from_seconds(settings.hit_flash_duration_s, TimerMode::Once),
            });
            if health.current <= 0.0 {
                let pos = enemy_tf.translation.truncate();
                commands.entity(enemy_entity).despawn();
                spawn_death_puff(&mut commands, &settings, pos);
                // Day 13-14: global hitstop on a killing blow. Insert
                // (or refresh) the resource — every combat/player/
                // enemy system gates on its presence, so the freeze
                // is felt uniformly across the game.
                commands.insert_resource(Hitstop {
                    timer: Timer::from_seconds(settings.hitstop_duration_s, TimerMode::Once),
                });
            }
            break;
        }
        if hit {
            commands.entity(proj_entity).despawn();
        }
    }
}
