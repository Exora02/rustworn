//! Shared component definitions, now split into domain sub-modules.
//!
//! Usage: `use crate::components::{Player, Health, Enemy, Room, ...};`
//! still works — everything is re-exported from this root.

pub mod arcgun; // marker for Arc Gun chain-lightning projectiles
pub mod enemy;
pub mod misc;
pub mod player;
pub mod synergy;
pub mod world;

// Re-exports from each sub-module so that existing `crate::components::*`
// imports continue to compile without changes.

pub use arcgun::ArcGunProjectile;
pub use enemy::{
    AiBehavior, Beacon, BomberState, Boss, BossPhase, BossProjectile, BossState, DamageNumber, DasherPhase,
    DasherState, DeathParticle, DeathPuff, Elite, Enemy, EnemyAI, EnemyArchetype, HitFlash, HitSpark,
    MotionTrail, OrbiterState, PickupPopup, ShooterState, SniperState, StatusEffects, TrailGhost,
    SkeletonArcherState, GhostState, SkullSwarmState,
    IceGolemState, SnowDrifterState, PenguinBomberState,
    FireMageState, LavaElemental, CinderWispState,
    TreantState, VineCreeperState, SporePodState,
    VoidWalkerState, CorruptionOrbState, ShadowCloneState,
};
pub use misc::{
    AppState, AutoAttackCooldowns, BaseColor, BeamVisual, Breakable, CascadeTelegraph,
    GameOverReason, Grenade, HudElement, HudElementKind, ItemKind, Pickup, PickupKind, Shrine,
    ShrineFocus, ShopItem, ShopReward, WeaponInventory,
};
pub use synergy::{check_new_synergies, synergy_name, SynergyEffect, SYNERGIES};
pub use player::{
    AimDirection, AimIndicator, BaseVelocity, CascadeAffected, Collider, DashCooldown, Health,
    Invulnerable, Knockback, LastMoveDirection, MainCamera, MeleeWeapon, Player, PlayerClass, Projectile,
    ReaverDashHit, Team, Velocity, Weapon, WeaponArchetype, PLAYER_COLOR,
};
pub use world::{
    Biome, biome_from_floor, ChallengeElite, ChallengeRun, ChallengeState, ChallengeWall,
    FloorTransitionText, Room, RoomId, RoomKind, SecretWall, TeleportPad, TreasurePedestal,
    Wall, WaveDefinition, WaveSpawner,
};