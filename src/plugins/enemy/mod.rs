//! Enemy plugin: spawner, AI, and contact-damage resolution.
//!
//! Module root for the enemy sub-system. System arrangement and
//! sub-module registration live here; actual behaviors are in
//! `spawn`, `ai`, `waves`, and `constants`.

pub mod ai;
pub mod boss_attacks;
pub mod spawn;
pub mod status;
pub mod waves;

use bevy::prelude::*;

pub use spawn::{
    spawn_beacon, spawn_boss, spawn_death_puff, GameSprites,
    spawn_skeleton_archer, spawn_ghost, spawn_skull_swarm,
    spawn_ice_golem, spawn_snow_drifter, spawn_penguin_bomber,
    spawn_fire_mage, spawn_lava_elemental, spawn_cinder_wisp,
    spawn_treant_enemy, spawn_vine_creeper, spawn_spore_pod,
    spawn_void_walker_enemy, spawn_corruption_orb, spawn_shadow_clone,
};

use crate::components::AppState;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn::load_sprites)
            .add_systems(
            Update,
            (
                waves::wave_spawner_tick_system,
                ai::enemy_ai_system,
                status::status_effect_tick_system,
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
