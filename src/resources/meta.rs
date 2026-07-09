//! Meta-progression: Crown currency, unlockable items/weapons, and persistence.
//! 
//! `CrownSave` is the persistent data saved between runs (loaded from
//! `crown_save.ron` on boot, saved on extraction and on unlockable purchases).
//! `MetaConfig` provides the in-memory lookup of unlockable costs/descriptions.
//!
//! ## Backward compatibility
//!
//! Old save files with `upgrades: [u32; 8]` are detected and converted to the
//! new `unlocks: [bool; 25]` format on load. A level > 0 in any old upgrade
//! slot becomes `true` (unlocked) in the new field.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Which unlockable slot — 25 variants total.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UnlockableKind {
    #[default]
    // Existing weapons (3)
    Beam = 0,
    Burst = 1,
    RocketLauncher = 2,
    // New weapons (3)
    Sniper = 3,
    SMG = 4,
    ArcGun = 5,
    // Passive items (10)
    SharpenedEdge = 6,
    RabbitsFoot = 7,
    TitaniumPlating = 8,
    Adrenaline = 9,
    Scope = 10,
    HeavyMagazine = 11,
    ConductingRod = 12,
    ReactiveArmor = 13,
    BloodPrice = 14,
    EchoChamber = 15,
    // Active items (4)
    SmokeBomb = 16,
    RageMode = 17,
    HealingAura = 18,
    TimeWarp = 19,
    // Existing passives (5)
    CrownOfGreed = 20,
    GlassCannon = 21,
    ExplosiveRounds = 22,
    Thorns = 23,
    Vampire = 24,
}

pub const UNLOCKABLE_COUNT: usize = 25;

/// Static definition of an unlockable — name, base cost, and description.
pub struct UnlockableDef {
    pub name: &'static str,
    pub base_cost: u32,
    pub description: &'static str,
}

/// Lookup table for unlockable costs and descriptions.
#[derive(Resource)]
pub struct MetaConfig {
    pub unlockables: [UnlockableDef; UNLOCKABLE_COUNT],
}

impl Default for MetaConfig {
    fn default() -> Self {
        Self {
            unlockables: [
                // Weapons (0-5)
                UnlockableDef { name: "Beam",              base_cost: 10, description: "Unlock Beam weapon" },
                UnlockableDef { name: "Burst",             base_cost: 10, description: "Unlock Burst weapon" },
                UnlockableDef { name: "Rocket Launcher",   base_cost: 15, description: "Unlock Rocket Launcher" },
                UnlockableDef { name: "Sniper",            base_cost: 18, description: "Unlock Sniper weapon" },
                UnlockableDef { name: "SMG",               base_cost: 12, description: "Unlock SMG weapon" },
                UnlockableDef { name: "Arc Gun",           base_cost: 18, description: "Unlock Arc Gun weapon" },
                // Passives (6-15)
                UnlockableDef { name: "Sharpened Edge",    base_cost: 15, description: "+1 pierce" },
                UnlockableDef { name: "Rabbit's Foot",     base_cost: 12, description: "+15% luck" },
                UnlockableDef { name: "Titanium Plating",  base_cost: 18, description: "+0.5 armor cap" },
                UnlockableDef { name: "Adrenaline",        base_cost: 15, description: "Speed boost when low HP" },
                UnlockableDef { name: "Scope",             base_cost: 14, description: "+15% range" },
                UnlockableDef { name: "Heavy Magazine",    base_cost: 20, description: "+20% firerate, -15% movespeed" },
                UnlockableDef { name: "Conducting Rod",    base_cost: 25, description: "Chain to 2 nearby enemies" },
                UnlockableDef { name: "Reactive Armor",    base_cost: 22, description: "Shockwave on damage taken" },
                UnlockableDef { name: "Blood Price",       base_cost: 18, description: "+1 HP start, +1 curse" },
                UnlockableDef { name: "Echo Chamber",      base_cost: 25, description: "Every 3rd shot doubled" },
                // Actives (16-19)
                UnlockableDef { name: "Smoke Bomb",        base_cost: 20, description: "Teleport dodge" },
                UnlockableDef { name: "Rage Mode",         base_cost: 25, description: "2x dmg for 3s" },
                UnlockableDef { name: "Healing Aura",      base_cost: 22, description: "Heal 2 HP over 3s" },
                UnlockableDef { name: "Time Warp",         base_cost: 30, description: "50% slow for 2s" },
                // Existing passives (20-24)
                UnlockableDef { name: "Crown of Greed",    base_cost: 20, description: "+50% gold" },
                UnlockableDef { name: "Glass Cannon",      base_cost: 25, description: "+2 DMG, 2 HP" },
                UnlockableDef { name: "Explosive Rounds",  base_cost: 20, description: "AoE projectiles" },
                UnlockableDef { name: "Thorns",            base_cost: 15, description: "Reflect damage" },
                UnlockableDef { name: "Vampire",           base_cost: 30, description: "Heal on kill" },
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
    pub unlocks: [bool; UNLOCKABLE_COUNT],
    /// Legacy field: kept for backward-compatible deserialization.
    /// If present and `unlocks` was not serialized, the field is
    /// converted on load. Never written back.
    #[serde(default)]
    pub upgrades: Option<[u32; 8]>,
}

fn default_unlocks() -> [bool; UNLOCKABLE_COUNT] {
    [false; UNLOCKABLE_COUNT]
}

impl Default for CrownSave {
    fn default() -> Self {
        Self {
            crown: 0,
            unlocks: [false; UNLOCKABLE_COUNT],
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
            if upgrades.len() == 8 && self.unlocks == [false; UNLOCKABLE_COUNT] {
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