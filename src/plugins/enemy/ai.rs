//! Enemy AI system — per-archetype behavior dispatch.
//!
//! Chaser: move toward player. Dasher: approach → dash → recover.
//! Shooter: maintain distance, fire projectiles. Sniper: charge &
//! fire. Boss: multi-phase. Orbiter: circle player, fire tangent.

use bevy::prelude::*;

use crate::components::{
    AiBehavior, BaseVelocity, BossPhase, BossState, DasherPhase, DasherState,
    Enemy, EnemyAI, Health, OrbiterState, Player,
    ShooterState, SniperState,
};
use crate::resources::Hitstop;
use crate::resources::{GameSettings, RunState};

use super::spawn::{
    spawn_boss_projectile, spawn_orbiter_projectile, spawn_shooter_projectile,
    spawn_sniper_projectile, SNIPER_CHARGED_COLOR, SNIPER_COLOR,
};

pub fn enemy_ai_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    _run: Res<RunState>,
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    mut enemies: Query<
        (
            &Transform,
            &EnemyAI,
            &mut BaseVelocity,
            &mut Sprite,
            &Health,
            Option<&mut DasherState>,
            Option<&mut ShooterState>,
            Option<&mut SniperState>,
            Option<&mut BossState>,
            Option<&mut OrbiterState>,
        ),
        With<Enemy>,
    >,
) {
    if hitstop.is_some() {
        return;
    }
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();

    for (
        enemy_transform,
        ai,
        mut base,
        mut sprite,
        health,
        dasher_state,
        shooter_state,
        sniper_state,
        boss_state,
        orbiter_state,
    ) in &mut enemies
    {
        match ai.behavior {
            AiBehavior::Chaser => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.chaser_move_speed;
            }
            AiBehavior::Bomber => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.bomber_move_speed;
            }
            AiBehavior::Dasher => {
                let Some(mut state) = dasher_state else {
                    continue;
                };
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                let (speed, next_phase, next_duration) = match state.phase {
                    DasherPhase::Approach => (
                        settings.dasher_approach_speed,
                        DasherPhase::Dash,
                        settings.dasher_dash_duration_s,
                    ),
                    DasherPhase::Dash => (
                        settings.dasher_dash_speed,
                        DasherPhase::Recover,
                        settings.dasher_recover_duration_s,
                    ),
                    DasherPhase::Recover => (
                        0.0,
                        DasherPhase::Approach,
                        settings.dasher_approach_duration_s,
                    ),
                };
                base.0 = dir * speed;
                state.phase_timer.tick(time.delta());
                if state.phase_timer.finished() {
                    state.phase = next_phase;
                    state.phase_timer = Timer::from_seconds(next_duration, TimerMode::Once);
                }
            }
            AiBehavior::Shooter => {
                let Some(mut state) = shooter_state else {
                    continue;
                };
                let enemy_pos = enemy_transform.translation.truncate();
                let to_player = player_pos - enemy_pos;
                let dist = to_player.length();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                let speed = if dist > settings.shooter_preferred_range {
                    settings.shooter_move_speed
                } else {
                    0.0
                };
                base.0 = dir * speed;
                state.fire_timer.tick(time.delta());
                if dist <= settings.shooter_preferred_range && state.fire_timer.finished() {
                    spawn_shooter_projectile(&mut commands, &settings, enemy_pos, dir);
                    let interval = 1.0 / settings.shooter_fire_rate;
                    state.fire_timer = Timer::from_seconds(interval, TimerMode::Once);
                }
            }
            AiBehavior::Sniper => {
                let Some(mut state) = sniper_state else {
                    continue;
                };
                let enemy_pos = enemy_transform.translation.truncate();
                let to_player = player_pos - enemy_pos;
                let dist = to_player.length();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                let speed = if dist > settings.sniper_preferred_range {
                    settings.sniper_move_speed
                } else {
                    0.0
                };
                base.0 = dir * speed;
                state.charge_timer.tick(time.delta());
                if !state.is_charging {
                    if state.charge_timer.finished() {
                        state.is_charging = true;
                        state.charge_timer =
                            Timer::from_seconds(settings.sniper_charge_duration_s, TimerMode::Once);
                    }
                } else {
                    let dur = state.charge_timer.duration().as_secs_f32();
                    if dur > 0.0 {
                        let progress =
                            (1.0 - state.charge_timer.remaining_secs() / dur).clamp(0.0, 1.0);
                        let base_srgba = SNIPER_COLOR.to_srgba();
                        let charged_srgba = SNIPER_CHARGED_COLOR.to_srgba();
                        sprite.color = Color::srgba(
                            base_srgba.red + (charged_srgba.red - base_srgba.red) * progress,
                            base_srgba.green + (charged_srgba.green - base_srgba.green) * progress,
                            base_srgba.blue + (charged_srgba.blue - base_srgba.blue) * progress,
                            base_srgba.alpha,
                        );
                    }
                    if state.charge_timer.finished() {
                        spawn_sniper_projectile(&mut commands, &settings, enemy_pos, dir);
                        state.is_charging = false;
                        state.charge_timer = Timer::from_seconds(
                            settings.sniper_post_fire_cooldown_s,
                            TimerMode::Once,
                        );
                        sprite.color = SNIPER_COLOR;
                    }
                }
            }
            AiBehavior::Boss => {
                let Some(mut state) = boss_state else {
                    continue;
                };
                let enemy_pos = enemy_transform.translation.truncate();
                let to_player = player_pos - enemy_pos;
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                state.current_hp = health.current;
                let half_hp = settings.boss_max_hp * 0.5;
                if state.phase == BossPhase::Phase1 && state.current_hp <= half_hp {
                    state.phase = BossPhase::Phase2;
                    state.attack_timer = Timer::from_seconds(
                        settings.boss_phase2_attack_interval_s,
                        TimerMode::Once,
                    );
                }
                let speed = match state.phase {
                    BossPhase::Phase1 => settings.boss_move_speed,
                    BossPhase::Phase2 => {
                        settings.boss_move_speed * settings.boss_phase2_speed_multiplier
                    }
                    BossPhase::Summoning | BossPhase::Charging | BossPhase::Teleporting => {
                        settings.boss_move_speed
                    }
                };
                base.0 = dir * speed;
                if state.phase == BossPhase::Phase2 {
                    state.attack_timer.tick(time.delta());
                    if state.attack_timer.finished() {
                        let n = settings.boss_phase2_burst_count.max(1) as i32;
                        let total_spread: f32 = 0.30;
                        for i in 0..n {
                            let t = if n == 1 {
                                0.0
                            } else {
                                i as f32 / (n - 1) as f32 - 0.5
                            };
                            let angle_offset = t * total_spread;
                            let (s, c) = angle_offset.sin_cos();
                            let burst_dir = Vec2::new(dir.x * c - dir.y * s, dir.x * s + dir.y * c);
                            spawn_boss_projectile(&mut commands, &settings, enemy_pos, burst_dir);
                        }
                        state.attack_timer = Timer::from_seconds(
                            settings.boss_phase2_attack_interval_s,
                            TimerMode::Once,
                        );
                    }
                }
            }
            AiBehavior::Beacon => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    -to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.beacon_flee_speed;
            }
            AiBehavior::Orbiter => {
                let Some(mut state) = orbiter_state else {
                    continue;
                };
                let enemy_pos = enemy_transform.translation.truncate();
                let dt = time.delta().as_secs_f32();
                let (sin, cos) = state.angle.sin_cos();
                let orbit_pos = player_pos + Vec2::new(cos, sin) * state.orbit_radius;
                let to_orbit = orbit_pos - enemy_pos;
                let dir = if to_orbit.length_squared() > 0.0 {
                    to_orbit.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.orbiter_move_speed;
                state.angle += state.angular_speed * dt;
                if state.angle > std::f32::consts::TAU {
                    state.angle -= std::f32::consts::TAU;
                }
                state.fire_timer.tick(time.delta());
                if state.fire_timer.finished() {
                    let tangent = Vec2::new(-sin, cos);
                    spawn_orbiter_projectile(&mut commands, &settings, enemy_pos, tangent);
                    state.fire_timer = Timer::from_seconds(
                        1.0 / settings.orbiter_fire_rate,
                        TimerMode::Repeating,
                    );
                }
            }
            // ─── Biome-specific enemies (simple Chaser-like movement) ──
            // These are simple movement-only entries; advanced attack
            // patterns (teleport, homing, split-on-death, AOE slam, etc.)
            // are handled by dedicated systems in the enemy plugin.
            AiBehavior::SkeletonArcher => {
                let enemy_pos = enemy_transform.translation.truncate();
                let to_player = player_pos - enemy_pos;
                let dist = to_player.length();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                let speed = if dist > settings.shooter_preferred_range {
                    settings.shooter_move_speed
                } else {
                    0.0
                };
                base.0 = dir * speed;
            }
            AiBehavior::Ghost => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.chaser_move_speed * 1.2;
            }
            AiBehavior::SkullSwarm => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.chaser_move_speed * 0.5;
            }
            AiBehavior::IceGolem => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.chaser_move_speed * 0.3;
            }
            AiBehavior::SnowDrifter => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.chaser_move_speed * 1.5;
            }
            AiBehavior::PenguinBomber => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.bomber_move_speed * 1.3;
            }
            AiBehavior::FireMage => {
                let enemy_pos = enemy_transform.translation.truncate();
                let to_player = player_pos - enemy_pos;
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.shooter_move_speed;
            }
            AiBehavior::LavaElemental => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.chaser_move_speed * 0.7;
            }
            AiBehavior::CinderWisp => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.chaser_move_speed * 1.8;
            }
            AiBehavior::Treant => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.chaser_move_speed * 0.25;
            }
            AiBehavior::VineCreeper => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.chaser_move_speed * 1.1;
            }
            AiBehavior::SporePod => {
                base.0 = Vec2::ZERO;
            }
            AiBehavior::VoidWalker => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.chaser_move_speed * 0.9;
            }
            AiBehavior::CorruptionOrb => {
                let enemy_pos = enemy_transform.translation.truncate();
                let to_player = player_pos - enemy_pos;
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.shooter_move_speed * 0.8;
            }
            AiBehavior::ShadowClone => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.chaser_move_speed * 0.8;
            }
            // Biome-specific enemy behaviors: all use chaser-like movement
            // as a fallback. Unique attack patterns are handled by their
            // respective per-enemy-type systems (status_effects, etc.).
            _ => {}
        }
    }
}