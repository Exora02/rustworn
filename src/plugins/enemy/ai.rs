//! Enemy AI system — the big dispatch-on-behavior match.
//!
//! For each enemy, dispatches on `AiBehavior` (Chaser, Dasher, Shooter,
//! Sniper, Boss, Beacon, Bomber, Orbiter) to compute their `BaseVelocity`
//! each frame. The match runs inside a `for` loop so every enemy gets
//! its own AI tick per frame.

use bevy::prelude::*;

use crate::components::{
    AiBehavior, BaseVelocity, BossPhase, BossState, DasherPhase, DasherState, Enemy, Player,
    ShooterState, SniperState,
};
use crate::resources::{GameSettings, Hitstop, RunState};

/// Ticks each enemy's AI: reads the player position, matches on behavior,
/// and writes `BaseVelocity` so the movement system can apply it.
pub fn enemy_ai_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
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
            _ => {}
        }
    }
}