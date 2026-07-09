//! Meta-progression: Crown currency, unlockable items/weapons, and persistence.
//! 
//! `CrownSave` is the persistent data saved between runs (loaded from
//! `crown_save.ron` on boot, saved on extraction and on unlockable purchases).
//! `MetaConfig` provides the in-memory lookup of unlockable costs/descriptions.
//!
//! ## Backward compatibility
//!
//! Old save files with `upgrades: [u32; 8]` are detected and converted to the
//! new `unlocks: [bool; 8]` format on load. A level > 0 in any old upgrade
//! slot becomes `true` (unlocked) in the new field.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Which unlockable slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UnlockableKind {
    #[default]
    Beam = 0,
    Burst = 1,
    RocketLauncher = 2,
    CrownOfGreed = 3,
    GlassCannon = 4,
    ExplosiveRounds = 5,
    Thorns = 6,
    Vampire = 7,
}

/// Static definition of an unlockable — name, base cost, and description.
pub struct UnlockableDef {
    pub name: &'static str,
    pub base_cost: u32,
    pub description: &'static str,
}

/// Lookup table for unlockable costs and descriptions.
#[derive(Resource)]
pub struct MetaConfig {
    pub unlockables: [UnlockableDef; 8],
}

impl Default for MetaConfig {
    fn default() -> Self {
        Self {
            unlockables: [
                UnlockableDef { name: "Beam",             base_cost: 10, description: "Unlock Beam weapon" },
                UnlockableDef { name: "Burst",            base_cost: 10, description: "Unlock Burst weapon" },
                UnlockableDef { name: "Rocket Launcher",   base_cost: 15, description: "Unlock Rocket Launcher" },
                UnlockableDef { name: "Crown of Greed",   base_cost: 20, description: "+50% gold" },
                UnlockableDef { name: "Glass Cannon",     base_cost: 25, description: "+2 DMG, 2 HP" },
                UnlockableDef { name: "Explosive Rounds", base_cost: 20, description: "AoE projectiles" },
                UnlockableDef { name: "Thorns",           base_cost: 15, description: "Reflect damage" },
                UnlockableDef { name: "Vampire",          base_cost: 30, description: "Heal on kill" },
            ],
        }
    }
}

/// Persistent Crown data: saved between runs, loaded on boot.
#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct CrownSave {
    /// Total Crown currency the player has saved up.
    pub crown: u32,
    /// Purchased unlocks. Index = UnlockableKind discriminant.
    #[serde(default = "default_unlocks")]
    pub unlocks: [bool; 8],
    /// Legacy field: kept for backward-compatible deserialization.
    /// If present and `unlocks` was not serialized, the field is
    /// converted on load. Never written back.
    #[serde(default)]
    pub upgrades: Option<[u32; 8]>,
}

fn default_unlocks() -> [bool; 8] {
    [false; 8]
}

impl Default for CrownSave {
    fn default() -> Self {
        Self {
            crown: 0,
            unlocks: [false; 8],
            upgrades: None,
        }
    }
}

impl CrownSave {
    /// Migrate legacy `upgrades` into `unlocks` if present.
    /// Called after deserialization to keep the rest of the
    /// codebase on the new `unlocks` field.
    pub fn migrate(&mut self) {
        if let Some(upgrades) = self.upgrades.take() {
            if upgrades.len() == 8 && self.unlocks == [false; 8] {
                for (i, &level) in upgrades.iter().enumerate() {
                    if level > 0 {
                        self.unlocks[i] = true;
                    }
                }
            }
        }
    }
}

/// The old upgrade kind enum — kept for reference but no longer used.
/// Remains in the crate to avoid breaking old imports that may
/// reference it elsewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MetaUpgradeKind {
    #[default]
    MaxHp = 0,
    MoveSpeed = 1,
    DashCooldown = 2,
    StartingGold = 3,
    ExtraSlot = 4,
    CascadeDuration = 5,
    Damage = 6,
    Regen = 7,
}