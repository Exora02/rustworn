//! Enemy plugin: spawner, AI, and contact-damage resolution.
//!
//! Module root for the enemy sub-system. System arrangement and
//! sub-module registration live here; actual behaviors are in
//! `spawn`, `ai`, `waves`, and `constants`.

pub mod ai;
pub mod boss_attacks;
pub mod constants;
pub mod spawn;
pub mod waves;

use bevy::prelude::*;

pub use spawn::{
    spawn_beacon, spawn_boss, spawn_death_puff,
};

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
                boss_attacks::lich_boss_system,
                boss_attacks::yeti_boss_system,
                boss_attacks::yeti_charge_movement_system,
                boss_attacks::inferno_boss_system,
                boss_attacks::treant_boss_system,
                boss_attacks::void_walker_boss_system,
            )
                .chain()
                .run_if(in_state(AppState::InRun)),
        );
    }
}
