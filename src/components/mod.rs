//! Shared component definitions, now split into domain sub-modules.
//!
//! Usage: `use crate::components::{Player, Health, Enemy, Room, ...};`
//! still works — everything is re-exported from this root.

pub mod enemy;
pub mod misc;
pub mod player;
pub mod world;

// Re-exports from each sub-module so that existing `crate::components::*`
// imports continue to compile without changes.

pub use enemy::{
    AiBehavior, Beacon, BomberState, Boss, BossPhase, BossProjectile, BossState, DamageNumber, DasherPhase,
    DasherState, DeathParticle, DeathPuff, Elite, Enemy, EnemyAI, EnemyArchetype, HitFlash, HitSpark,
    MotionTrail, OrbiterState, PickupPopup, ShooterState, SniperState, StatusEffects, TrailGhost,
};
pub use misc::{
    AppState, AutoAttackCooldowns, BaseColor, BeamVisual, Breakable, CascadeTelegraph,
    GameOverReason, Grenade, HudElement, HudElementKind, Pickup, PickupKind, Shrine, ShrineFocus,
    ShopItem, ShopReward, WeaponInventory,
};
pub use player::{
    AimDirection, AimIndicator, BaseVelocity, CascadeAffected, Collider, DashCooldown, Health,
    Invulnerable, Knockback, LastMoveDirection, MainCamera, MeleeWeapon, Player, PlayerClass, Projectile,
    ReaverDashHit, Team, Velocity, Weapon, WeaponArchetype, PLAYER_COLOR,
};
pub use world::{
    Biome, biome_from_floor, FloorTransitionText, Room, RoomId, RoomKind, TeleportPad, Wall,
    WaveDefinition, WaveSpawner,
};