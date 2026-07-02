//! Enemy plugin: spawner, AI, and contact-damage resolution.
//!
//! Module root for the enemy sub-system. System arrangement and
//! sub-module registration live here; actual behaviors are in
//! `spawn`, `ai`, `waves`, and `constants`.

pub mod ai;
pub mod constants;
pub mod spawn;
pub mod waves;

use bevy::prelude::*;

pub use ai::enemy_ai_system;
pub use spawn::{
    death_puff_tick_system, hit_flash_tick_system, spawn_beacon, spawn_boss, spawn_death_puff,
};
pub use waves::wave_spawner_tick_system;

use crate::components::AppState;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                waves::wave_spawner_tick_system,
                ai::enemy_ai_system,
                spawn::death_puff_tick_system,
                spawn::hit_flash_tick_system,
            )
                .chain()
                .run_if(in_state(AppState::InRun)),
        );
    }
}
