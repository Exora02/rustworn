//! Per-biome boss attack systems.
//!
//! Each biome boss has a unique attack pattern. These systems run
//! in Update gated on `AppState::InRun`. They read the boss entity's
//! `Transform` + `BossState` and the player's `Transform`, then
//! spawn `Team::Enemy` projectiles with timed lifetimes.
//!
//! Each boss has two attack patterns:
//! - Pattern 1: runs at all HP levels (the existing attack)
//! - Pattern 2: health-gated — only activates when HP ≤ 50% max,
//!   using `BossState::phase2_attack_timer`

use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

use crate::components::{
    BaseVelocity, Boss, BossProjectile, BossState, Collider, Enemy, Health, Player,
    Projectile, Team, Velocity,
};
use crate::resources::GameSettings;

/// Shared helper: spawn a small enemy-aligned projectile from `origin`
/// moving in `dir` at `speed` with `damage`, coloured `color`.
fn spawn_boss_proj(
    commands: &mut Commands,
    origin: Vec2,
    dir: Vec2,
    speed: f32,
    damage: f32,
    color: Color,
    lifetime_s: f32,
) {
    let spawn_pos = origin + dir * 25.0;
    commands.spawn((
        Sprite {
            color,
            custom_size: Some(Vec2::splat(8.0)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5),
        Velocity(dir * speed),
        BaseVelocity(dir * speed),
        Collider {
            half_extents: Vec2::splat(4.0),
        },
        Projectile {
            damage,
            lifetime: Timer::from_seconds(lifetime_s, TimerMode::Once),
            team: Team::Enemy,
        },
        BossProjectile {
            lifetime: Timer::from_seconds(lifetime_s, TimerMode::Once),
        },
    ));
}

/// Spawn an AOE visual puff (non-damaging) at `pos` with `color`
/// that fades quickly. Used for telegraphing ground-pound, ice-ring, etc.
fn spawn_aoe_puff(commands: &mut Commands, pos: Vec2, color: Color, size: f32, lifetime_s: f32) {
    commands.spawn((
        Sprite {
            color: color.with_alpha(0.4),
            custom_size: Some(Vec2::splat(size)),
            ..default()
        },
        Transform::from_xyz(pos.x, pos.y, 0.3),
        BossProjectile {
            lifetime: Timer::from_seconds(lifetime_s, TimerMode::Once),
        },
    ));
}

// ─── Helper: check whether the boss is in Phase 2 (HP ≤ 50%) ─────────

fn is_phase2(health: &Health) -> bool {
    health.current <= health.max * 0.5
}

/// ─── Lich (Catacombs) ────────────────────────────────────────────────
///
/// Pattern 1 (all HP): Every 3s, fire 3 spread projectiles toward the
///   player (17° spread, matching the Phase 2 boss burst arc).
///
/// Pattern 2 (<50% HP): Every 5s, summon 2 chaser enemies near the boss.
pub fn lich_boss_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    mut boss_query: Query<(&Transform, &Health, &mut BossState), (With<Boss>, With<Enemy>)>,
) {
    let Ok(player_tf) = player_query.get_single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();

    for (boss_tf, health, mut state) in &mut boss_query {
        let boss_pos = boss_tf.translation.truncate();

        // ── Pattern 1: 3-burst spread (all HP levels) ─────────────
        state.attack_timer.tick(time.delta());
        if state.attack_timer.finished() {
            let to_player = player_pos - boss_pos;
            let dir = if to_player.length_squared() > 0.0 {
                to_player.normalize()
            } else {
                Vec2::ZERO
            };

            // 3-burst spread, 17° total arc.
            let n = 3;
            let total_spread: f32 = 0.30; // ~17°
            for i in 0..n {
                let t = if n == 1 {
                    0.0
                } else {
                    i as f32 / (n - 1) as f32 - 0.5
                };
                let angle_offset = t * total_spread;
                let (s, c) = angle_offset.sin_cos();
                let burst_dir = Vec2::new(dir.x * c - dir.y * s, dir.x * s + dir.y * c);
                spawn_boss_proj(
                    &mut commands,
                    boss_pos,
                    burst_dir,
                    settings.boss_projectile_speed,
                    1.0,
                    Color::srgb(0.75, 0.30, 0.75), // purple projectile
                    3.0,
                );
            }
            state.attack_timer = Timer::from_seconds(3.0, TimerMode::Repeating);
        }

        // ── Pattern 2: Summon 2 chasers (HP ≤ 50%) ───────────────
        if is_phase2(health) {
            state.phase2_attack_timer.tick(time.delta());
            if state.phase2_attack_timer.finished() {
                // Spawn 2 chasers on opposite sides near the boss
                for i in 0..2 {
                    let angle = i as f32 * PI; // 0 and π
                    let offset = Vec2::new(angle.cos(), angle.sin()) * 40.0;
                    let spawn_pos = boss_pos + offset;

                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.84, 0.16, 0.16), // chaser red
                            custom_size: Some(Vec2::splat(20.0)),
                            ..default()
                        },
                        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
                        Enemy,
                        crate::components::EnemyAI {
                            behavior: crate::components::AiBehavior::Chaser,
                        },
                        Collider {
                            half_extents: Vec2::splat(10.0),
                        },
                        Health::new(3.0),
                        Velocity::default(),
                        BaseVelocity::default(),
                        crate::components::CascadeAffected,
                        crate::components::BaseColor(Color::srgb(0.84, 0.16, 0.16)),
                    ));
                }

                state.phase2_attack_timer = Timer::from_seconds(5.0, TimerMode::Repeating);
            }
        }
    }
}

/// ─── Yeti (Frost) ────────────────────────────────────────────────────
///
/// Pattern 1 (all HP): Every 5s, charge at the player's current position
///   (high-speed dash with boss-level knockback).
///
/// Pattern 2 (<50% HP): Ice ring — spawn 4 projectiles in a circle
///   around the boss, moving outward.
pub fn yeti_boss_system(
    time: Res<Time>,
    _settings: Res<GameSettings>,
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    mut boss_query: Query<
        (Entity, &Transform, &Health, &mut BossState),
        (With<Boss>, With<Enemy>),
    >,
) {
    let Ok(player_tf) = player_query.get_single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();

    for (entity, boss_tf, health, mut state) in &mut boss_query {
        let boss_pos = boss_tf.translation.truncate();

        // ── Pattern 1: Charge (all HP levels) ─────────────────────
        state.attack_timer.tick(time.delta());
        if state.attack_timer.finished() {
            let to_player = player_pos - boss_pos;
            let dir = if to_player.length_squared() > 0.0 {
                to_player.normalize()
            } else {
                Vec2::ZERO
            };

            // Charge: apply high velocity for 0.3s using a temporary component.
            commands.entity(entity).insert(Charging {
                timer: Timer::from_seconds(0.3, TimerMode::Once),
                direction: dir,
            });
            state.attack_timer = Timer::from_seconds(5.0, TimerMode::Repeating);
        }

        // ── Pattern 2: Ice ring (HP ≤ 50%) ───────────────────────
        if is_phase2(health) {
            state.phase2_attack_timer.tick(time.delta());
            if state.phase2_attack_timer.finished() {
                // Spawn 4 projectiles in a circle (every 90°)
                for i in 0..4 {
                    let angle = i as f32 * TAU / 4.0;
                    let dir = Vec2::new(angle.cos(), angle.sin());
                    spawn_boss_proj(
                        &mut commands,
                        boss_pos,
                        dir,
                        180.0,
                        1.0,
                        Color::srgb(0.50, 0.80, 1.0), // ice blue
                        4.0,
                    );
                }

                // Visual telegraph — brief ice puff at boss
                spawn_aoe_puff(&mut commands, boss_pos, Color::srgb(0.50, 0.80, 1.0), 60.0, 0.3);

                state.phase2_attack_timer = Timer::from_seconds(5.0, TimerMode::Repeating);
            }
        }
    }
}

/// Temporary component for Yeti charge state.
#[derive(Component)]
pub(crate) struct Charging {
    timer: Timer,
    direction: Vec2,
}

/// Apply Yeti charge velocity each frame while `Charging` exists.
pub fn yeti_charge_movement_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Charging, &mut Velocity)>,
) {
    for (entity, mut charge, mut vel) in &mut query {
        charge.timer.tick(time.delta());
        if charge.timer.finished() {
            commands.entity(entity).remove::<Charging>();
            vel.0 = Vec2::ZERO;
        } else {
            vel.0 = charge.direction * 400.0;
        }
    }
}

/// ─── Inferno (Magma) ─────────────────────────────────────────────────
///
/// Pattern 1 (all HP): Every 4s, lob an explosive projectile toward the
///   player position.
///
/// Pattern 2 (<50% HP): Ground pound — AOE circle around boss, pushes
///   player away. Spawns a visual shockwave ring then projects damage
///   outward.
pub fn inferno_boss_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    mut boss_query: Query<(&Transform, &Health, &mut BossState), (With<Boss>, With<Enemy>)>,
) {
    let Ok(player_tf) = player_query.get_single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();

    for (boss_tf, health, mut state) in &mut boss_query {
        let boss_pos = boss_tf.translation.truncate();

        // ── Pattern 1: Lob explosive projectile (all HP levels) ───
        state.attack_timer.tick(time.delta());
        if state.attack_timer.finished() {
            let to_player = player_pos - boss_pos;
            let dir = if to_player.length_squared() > 0.0 {
                to_player.normalize()
            } else {
                Vec2::ZERO
            };

            // Lob projectile: slower, bigger, orange.
            let spawn_pos = boss_pos + dir * 30.0;
            commands.spawn((
                Sprite {
                    color: Color::srgb(1.0, 0.65, 0.0),
                    custom_size: Some(Vec2::splat(14.0)),
                    ..default()
                },
                Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5),
                Velocity(dir * 150.0),
                BaseVelocity(dir * 150.0),
                Collider {
                    half_extents: Vec2::splat(7.0),
                },
                Projectile {
                    damage: settings.boss_projectile_damage * 1.5,
                    lifetime: Timer::from_seconds(3.0, TimerMode::Once),
                    team: Team::Enemy,
                },
                BossProjectile {
                    lifetime: Timer::from_seconds(3.0, TimerMode::Once),
                },
            ));
            state.attack_timer = Timer::from_seconds(4.0, TimerMode::Repeating);
        }

        // ── Pattern 2: Ground pound (HP ≤ 50%) ───────────────────
        if is_phase2(health) {
            state.phase2_attack_timer.tick(time.delta());
            if state.phase2_attack_timer.finished() {
                // Spawn 8 projectiles in a shockwave ring around boss
                let n = 8;
                for i in 0..n {
                    let angle = i as f32 * TAU / n as f32;
                    let dir = Vec2::new(angle.cos(), angle.sin());

                    let spawn_pos = boss_pos + dir * 30.0;
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(1.0, 0.40, 0.0), // bright orange
                            custom_size: Some(Vec2::splat(10.0)),
                            ..default()
                        },
                        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5),
                        Velocity(dir * 200.0),
                        BaseVelocity(dir * 200.0),
                        Collider {
                            half_extents: Vec2::splat(5.0),
                        },
                        Projectile {
                            damage: 1.0,
                            lifetime: Timer::from_seconds(1.5, TimerMode::Once),
                            team: Team::Enemy,
                        },
                        BossProjectile {
                            lifetime: Timer::from_seconds(1.5, TimerMode::Once),
                        },
                    ));
                }

                // Visual telegraph — ground-pound puff
                spawn_aoe_puff(
                    &mut commands,
                    boss_pos,
                    Color::srgb(1.0, 0.40, 0.0),
                    80.0,
                    0.3,
                );

                state.phase2_attack_timer = Timer::from_seconds(6.0, TimerMode::Repeating);
            }
        }
    }
}

/// ─── Treant (Verdant) ────────────────────────────────────────────────
///
/// Pattern 1 (all HP): Every 6s, heal 2 HP and spawn 2 orbiting
///   projectiles (small green bullets that orbit the boss briefly,
///   then despawn).
///
/// Pattern 2 (<50% HP): Root — briefly root the player in place for
///   0.5s. Spawns vines at the player's current position as a visual
///   indicator.
pub fn treant_boss_system(
    time: Res<Time>,
    _settings: Res<GameSettings>,
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    mut boss_query: Query<(&Transform, &mut Health, &mut BossState), (With<Boss>, With<Enemy>)>,
) {
    let Ok(player_tf) = player_query.get_single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();

    for (boss_tf, mut health, mut state) in &mut boss_query {
        // ── Pattern 1: Heal + orbiting projectiles (all HP levels) ─
        state.attack_timer.tick(time.delta());
        if state.attack_timer.finished() {
            // Heal 2 HP.
            health.current = (health.current + 2.0).min(health.max);

            // Spawn 2 orbiting projectiles at opposite angles.
            let boss_pos = boss_tf.translation.truncate();
            let angles = [0.0, std::f32::consts::PI];
            for angle in angles {
                let dir = Vec2::new(angle.cos(), angle.sin());
                let spawn_pos = boss_pos + dir * 35.0;
                commands.spawn((
                    Sprite {
                        color: Color::srgb(0.30, 0.85, 0.30),
                        custom_size: Some(Vec2::splat(6.0)),
                        ..default()
                    },
                    Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5),
                    Velocity(dir * 60.0),
                    BaseVelocity(dir * 60.0),
                    Collider {
                        half_extents: Vec2::splat(3.0),
                    },
                    Projectile {
                        damage: 1.0,
                        lifetime: Timer::from_seconds(3.0, TimerMode::Once),
                        team: Team::Enemy,
                    },
                    BossProjectile {
                        lifetime: Timer::from_seconds(3.0, TimerMode::Once),
                    },
                ));
            }
            state.attack_timer = Timer::from_seconds(6.0, TimerMode::Repeating);
        }

        // ── Pattern 2: Root (HP ≤ 50%) ───────────────────────────
        if is_phase2(&health) {
            state.phase2_attack_timer.tick(time.delta());
            if state.phase2_attack_timer.finished() {
                // Spawn root-vines at the player's current position.
                // These are visual + damaging projectiles that linger.
                for i in 0..3 {
                    let offset = Vec2::new(
                        (i as f32 - 1.0) * 15.0,
                        5.0,
                    );
                    let vine_pos = player_pos + offset;
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.15, 0.55, 0.10), // deep green
                            custom_size: Some(Vec2::new(10.0, 18.0)),
                            ..default()
                        },
                        Transform::from_xyz(vine_pos.x, vine_pos.y, 0.5),
                        Velocity::default(),
                        BaseVelocity::default(),
                        Collider {
                            half_extents: Vec2::new(5.0, 9.0),
                        },
                        Projectile {
                            damage: 0.5, // minor tick damage
                            lifetime: Timer::from_seconds(0.5, TimerMode::Once),
                            team: Team::Enemy,
                        },
                        BossProjectile {
                            lifetime: Timer::from_seconds(0.5, TimerMode::Once),
                        },
                    ));
                }

                state.phase2_attack_timer = Timer::from_seconds(7.0, TimerMode::Repeating);
            }
        }
    }
}

/// ─── Void Walker (Void) ──────────────────────────────────────────────
///
/// Pattern 1 (all HP): Every 3s, teleport to a random position near the
///   boss's current location and fire a homing (direct-aim) projectile
///   at the player.
///
/// Pattern 2 (<50% HP): Void rift — spawn 3 projectiles from random
///   positions toward the player in quick succession (fired over a
///   short burst).
pub fn void_walker_boss_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    mut boss_query: Query<
        (&mut Transform, &Health, &mut BossState, &mut Sprite),
        (With<Boss>, With<Enemy>),
    >,
) {
    let Ok(player_tf) = player_query.get_single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();

    for (mut boss_tf, health, mut state, mut sprite) in &mut boss_query {
        // ── Pattern 1: Teleport + homing projectile (all HP levels) ─
        state.attack_timer.tick(time.delta());
        if state.attack_timer.finished() {
            // Teleport to a random nearby position (within 80px, clamped).
            let origin = boss_tf.translation.truncate();
            let angle = state.attack_timer.duration().as_secs_f32() * 1.7
                + state.attack_timer.elapsed_secs() * 3.1;
            let offset = Vec2::new(angle.cos(), angle.sin()) * 60.0;
            let new_pos = origin + offset;
            boss_tf.translation.x = new_pos.x;
            boss_tf.translation.y = new_pos.y;

            // Purple flash on teleport.
            sprite.color = Color::srgb(0.50, 0.10, 0.60);
            // (The hit_flash system will snap it back next frame)

            // Fire a homing projectile toward player.
            let to_player = player_pos - new_pos;
            let dir = if to_player.length_squared() > 0.0 {
                to_player.normalize()
            } else {
                Vec2::ZERO
            };

            let spawn_pos = new_pos + dir * 25.0;
            commands.spawn((
                Sprite {
                    color: Color::srgb(0.50, 0.10, 0.60),
                    custom_size: Some(Vec2::splat(10.0)),
                    ..default()
                },
                Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5),
                Velocity(dir * settings.boss_projectile_speed * 1.2),
                BaseVelocity(dir * settings.boss_projectile_speed * 1.2),
                Collider {
                    half_extents: Vec2::splat(5.0),
                },
                Projectile {
                    damage: settings.boss_projectile_damage,
                    lifetime: Timer::from_seconds(3.0, TimerMode::Once),
                    team: Team::Enemy,
                },
                BossProjectile {
                    lifetime: Timer::from_seconds(3.0, TimerMode::Once),
                },
            ));
            state.attack_timer = Timer::from_seconds(3.0, TimerMode::Repeating);
        }

        // ── Pattern 2: Void rift (HP ≤ 50%) ──────────────────────
        if is_phase2(health) {
            state.phase2_attack_timer.tick(time.delta());
            if state.phase2_attack_timer.finished() {
                // Fire 3 projectiles from random positions around
                // the boss, all aimed toward the player.
                for i in 0..3 {
                    // Each projectile emerges from a different angle
                    // around the boss.
                    let rift_angle = i as f32 * TAU / 3.0 + state.phase2_attack_timer.elapsed_secs();
                    let rift_offset = Vec2::new(rift_angle.cos(), rift_angle.sin()) * 50.0;
                    let rift_pos = boss_tf.translation.truncate() + rift_offset;

                    let to_player = player_pos - rift_pos;
                    let dir = if to_player.length_squared() > 0.0 {
                        to_player.normalize()
                    } else {
                        Vec2::ZERO
                    };

                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.40, 0.05, 0.50),
                            custom_size: Some(Vec2::splat(8.0)),
                            ..default()
                        },
                        Transform::from_xyz(rift_pos.x, rift_pos.y, 0.5),
                        Velocity(dir * settings.boss_projectile_speed),
                        BaseVelocity(dir * settings.boss_projectile_speed),
                        Collider {
                            half_extents: Vec2::splat(4.0),
                        },
                        Projectile {
                            damage: settings.boss_projectile_damage,
                            lifetime: Timer::from_seconds(2.5, TimerMode::Once),
                            team: Team::Enemy,
                        },
                        BossProjectile {
                            lifetime: Timer::from_seconds(2.5, TimerMode::Once),
                        },
                    ));
                }

                // Visual telegraph — brief dark puff at each rift point
                for i in 0..3 {
                    let rift_angle = i as f32 * TAU / 3.0 + state.phase2_attack_timer.elapsed_secs();
                    let rift_offset = Vec2::new(rift_angle.cos(), rift_angle.sin()) * 50.0;
                    let rift_pos = boss_tf.translation.truncate() + rift_offset;
                    spawn_aoe_puff(
                        &mut commands,
                        rift_pos,
                        Color::srgb(0.30, 0.05, 0.40),
                        20.0,
                        0.3,
                    );
                }

                state.phase2_attack_timer = Timer::from_seconds(4.0, TimerMode::Repeating);
            }
        }
    }
}