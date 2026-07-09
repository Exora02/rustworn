//! Meta-progression: Crown currency, upgrade tree, and persistence.
//! 
//! `CrownSave` is the persistent data saved between runs (loaded from
//! `crown_save.ron` on boot, saved on extraction and on upgrade purchases).
//! `MetaConfig` provides the in-memory lookup of upgrade costs/effects.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Persistent Crown data: saved between runs, loaded on boot.
#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct CrownSave {
    /// Total Crown currency the player has saved up.
    pub crown: u32,
    /// Purchased upgrades. Index = MetaUpgradeKind discriminant.
    pub upgrades: [u32; 8],
}

impl Default for CrownSave {
    fn default() -> Self {
        Self {
            crown: 0,
            upgrades: [0; 8],
        }
    }
}

/// Which upgrade slot.
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

/// Lookup table for upgrade costs and effects.
#[derive(Resource)]
pub struct MetaConfig {
    pub upgrades: [MetaUpgradeDef; 8],
}

pub struct MetaUpgradeDef {
    pub kind: MetaUpgradeKind,
    pub name: &'static str,
    pub description: &'static str,
    pub base_cost: u32,
    pub cost_per_level: u32,
    pub max_level: u32,
}

impl Default for MetaConfig {
    fn default() -> Self {
        Self {
            upgrades: [
                MetaUpgradeDef { kind: MetaUpgradeKind::MaxHp, name: "Max HP", description: "+1 max HP", base_cost: 15, cost_per_level: 0, max_level: 3 },
                MetaUpgradeDef { kind: MetaUpgradeKind::MoveSpeed, name: "Move Speed", description: "+10% move speed", base_cost: 20, cost_per_level: 0, max_level: 3 },
                MetaUpgradeDef { kind: MetaUpgradeKind::DashCooldown, name: "Dash Cd", description: "-0.2s cooldown", base_cost: 15, cost_per_level: 0, max_level: 3 },
                MetaUpgradeDef { kind: MetaUpgradeKind::StartingGold, name: "Starting Gold", description: "Start with 10g", base_cost: 10, cost_per_level: 0, max_level: 2 },
                MetaUpgradeDef { kind: MetaUpgradeKind::ExtraSlot, name: "Extra Slot", description: "5th weapon slot", base_cost: 50, cost_per_level: 0, max_level: 1 },
                MetaUpgradeDef { kind: MetaUpgradeKind::CascadeDuration, name: "Cascade Dur", description: "+3s per level", base_cost: 25, cost_per_level: 0, max_level: 2 },
                MetaUpgradeDef { kind: MetaUpgradeKind::Damage, name: "Damage", description: "+5% damage", base_cost: 30, cost_per_level: 0, max_level: 3 },
                MetaUpgradeDef { kind: MetaUpgradeKind::Regen, name: "Regen", description: "+0.5 HP/s", base_cost: 25, cost_per_level: 0, max_level: 2 },
            ],
        }
    }
}