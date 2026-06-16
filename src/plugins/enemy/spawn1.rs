//! Enemy spawn systems.
//!
//! Generates enemy entities from wave definitions and spawns
//! individual archetypes with their full component bundles.

use bevy::prelude::*;
use log::info;

use crate::components::{
    AiBehavior, BaseColor, BaseVelocity, Beacon, BomberState, Boss, BossPhase, BossState,
    CascadeAffected, Collider, DasherPhase, DasherState, DeathPuff, Elite, Enemy, EnemyAI,
    EnemyArchetype, Health, HitFlash, Invulnerable, Knockback, MotionTrail, OrbiterState,
    Player, Projectile, Room, RoomKind, ShooterState, SniperState, Team, Velocity, WaveSpawner,
};
use crate::events::{DamageEvent, RoomClearedEvent};
use crate::plugins::fx::spawn_hit_spark;
use crate::resources::{BossRampTimer, FloorState, GameSettings, Hitstop, RunRng, RunState, ScreenShake};

// ─── Spawn helpers ──────────────────────────────────────────────────────────

pub fn floor_difficulty_mult(floor: u32) -> (f32, f32) {
    let hp_mult = 1.0 + (floor as f32).min(10.0) * 0.05;
    let dmg_mult = 1.0 + (floor as f32).min(10.0) * 0.03;
    (hp_mult, dmg_mult)
}

pub fn spawn_chaser(
    commands: &mut Commands,
    settings: &GameSettings,
    room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.chaser_max_hp * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite { CHASER_COLOR } else { CHASER_COLOR };
    // ... etc
}

// etc.