//! Enemy spawn helpers: all `fn spawn_*` for creating enemy entities.
//!
//! Each `spawn_*` function creates a bundle of components (Sprite, Transform,
//! EnemyAI, Collider, Health, Velocity, etc.) and returns the entity.
//! Called by the wave system and by the boss timer.

use bevy::prelude::*;
use log::info;

use crate::components::{
    AiBehavior, BaseColor, BaseVelocity, Boss, CascadeAffected, Collider, DasherPhase, DasherState,
    DeathPuff, Elite, Enemy, EnemyAI, EnemyArchetype, Health, HitFlash, Invulnerable, Knockback,
    MotionTrail, Player, Projectile, Room, RoomKind, ShooterState, SniperState, Team, Velocity,
    WaveSpawner, Beacon,
};
use crate::events::DamageEvent;
use crate::resources::{GameSettings, RunRng, RunState, ScreenShake};

// ─── Constants ──────────────────────────────────────────────────────────────

pub const CHASER_COLOR: Color = Color::srgb(0.84, 0.16, 0.16);
pub const DASHER_COLOR: Color = Color::srgb(0.62, 0.20, 0.85);
pub const SHOOTER_COLOR: Color = Color::srgb(0.20, 0.75, 0.75);
pub const SHOOTER_PROJECTILE_COLOR: Color = Color::srgb(0.45, 0.90, 0.90);
pub const SNIPER_COLOR: Color = Color::srgb(0.95, 0.85, 0.25);
pub const SNIPER_CHARGED_COLOR: Color = Color::srgb(0.95, 0.20, 0.15);
pub const BOMBER_COLOR: Color = Color::srgb(0.95, 0.60, 0.15);
pub const ORBITER_COLOR: Color = Color::srrgb(0.20, 0.80, 0.90);
pub const BOMBER_EXPLOSION_COLOR: Color = Color::srgb(1.0, 0.40, 0.10);
pub const BOSS_COLOR: Color = Color::srgb(0.70, 0.05, 0.05);
pub const BOSS_PROJECTILE_COLOR: Color = Color::srgb(0.95, 0.20, 0.15);
pub const BEACON_COLOR: Color = Color::srgb(1.0, 0.85, 0.20);
pub const ELITE_TINT: Color = Color::srgb(0.95, 0.78, 0.12);

pub const BOMBER_SIZE: f32 = 20.0;
pub const ORBITER_SIZE: f32 = 14.0;
pub const CHASER_SIZE: f32 = 30.0;
pub const DASHER_SIZE: f32 = 30.0;
pub const SHOOTER_SIZE: f32 = 30.0;
pub const SNIPER_SIZE: f32 = 30.0;
pub const BOSS_SIZE: f32 = 50.0;

pub const ENEMY_COST_CHASER: f32 = 5.0;
pub const ENEMY_COST_DASHER: f32 = 8.0;
pub const ENEMY_COST_SHOOTER: f32 = 7.0;
pub const ENEMY_COST_SNIPER: f32 = 12.0;
pub const ENEMY_COST_BOMBER: f32 = 8.0;
pub const ENEMY_COST_ORBITER: f32 = 10.0;
pub const ELITE_COST_MULTIPLIER: f32 = 2.0;
pub const CREDIT_RATE_FIGHT: f32 = 8.0;
pub const CREDIT_RATE_HORDE: f32 = 10.0;
pub const CREDIT_RATE_ELITE: f32 = 6.0;
pub const CREDIT_RATE_BOSS: f32 = 5.0;
pub const CREDITS_MAX: f32 = 120.0;

pub fn floor_difficulty_mult(floor: u32) -> (f32, f32) {
    let hp_mult = 1.0 + (floor as f32).min(10.0) * 0.05;
    let dmg_mult = 1.0 + (floor as f32).min(10.0) * 0.03;
    (hp_mult, dmg_mult)
}