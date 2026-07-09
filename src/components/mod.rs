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
    AiBehavior, Beacon, BomberState, Boss, BossPhase, BossState, DamageNumber, DasherPhase,
    DasherState, DeathPuff, Elite, Enemy, EnemyAI, EnemyArchetype, HitFlash, HitSpark,
    MotionTrail, OrbiterState, ShooterState, SniperState, TrailGhost,
};
pub use misc::{
    AppState, AutoAttackCooldowns, BaseColor, BeamVisual, Breakable, CascadeTelegraph,
    GameOverReason, HudElement, HudElementKind, Pickup, PickupKind, Shrine, ShrineFocus, ShopItem,
    ShopReward, WeaponInventory,
};
pub use player::{
    AimDirection, AimIndicator, BaseVelocity, CascadeAffected, Collider, DashCooldown, Health,
    Invulnerable, Knockback, LastMoveDirection, MainCamera, Player, Projectile, Team, Velocity,
    Weapon, WeaponArchetype, PLAYER_COLOR,
};
pub use world::{
    Door, FloorTransitionText, Room, RoomId, RoomKind, RoomScope, TeleportPad, Wall,
    WaveDefinition, WaveSpawner,
};