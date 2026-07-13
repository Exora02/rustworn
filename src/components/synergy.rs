//! Item synergy system — Isaac-style pair combos.
//!
//! When a player holds two specific items, a hidden synergy effect
//! activates. Synergies are discovered naturally by the player:
//! no UI tells them the table.

use crate::components::ItemKind;

/// All possible synergy effects. Each maps to one pair of items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SynergyEffect {
    /// SteelToe + RustyNails: projectiles with pierce pierce ALL enemies
    /// (skip pierce counter decrement).
    SuperPierce,
    /// LuckyCharm + LuckyHorseshoe: every 3rd hit auto-crits.
    GuaranteedCrit,
    /// VampireFangs + RustyNails: low-HP lifesteal is 3× instead of 2×.
    BloodLifesteal,
    /// Featherlight + ExplosiveCharge: chain lightning hits chain an
    /// additional time (2 targets → 4).
    EchoChain,
    /// ExplosiveCharge + EnergyShield: getting hit spawns an explosion
    /// centered on the player.
    ExplosiveRetaliation,
    /// GlassCannon + VampireFangs: 2× damage output, but take 1 damage
    /// on every kill.
    DoubleEdge,
    /// Featherlight + SteelToe: ignore move-speed debuffs from items.
    SpeedShoot,
    /// EnergyShield + Bandage: healing overfills shield up to +50%.
    OverShield,
    /// CursedRing + Bandage: curse also gives +0.5 HP regen.
    RegenerativeArmor,
    /// MagnifyingGlass + LuckyCharm: luck increases beam/projectile width.
    BeamWide,
    /// CursedRing + ExplosiveCharge: rockets have 2× explosion radius.
    BigBoom,
    /// GlassCannon + CursedRing: curse boosts crit damage (×2 → ×3).
    CriticalCurse,
}

/// Static synergy table: 12 item pair → effect mappings.
/// Uses only items that exist in the current ItemKind enum.
pub const SYNERGIES: &[(ItemKind, ItemKind, SynergyEffect)] = &[
    // ── Fun impactful combos ──────────────────────────────────────────
    (ItemKind::SteelToe, ItemKind::RustyNails, SynergyEffect::SuperPierce),
    (ItemKind::ExplosiveCharge, ItemKind::EnergyShield, SynergyEffect::ExplosiveRetaliation),
    (ItemKind::Featherlight, ItemKind::ExplosiveCharge, SynergyEffect::EchoChain),
    (ItemKind::GlassCannon, ItemKind::VampireFangs, SynergyEffect::DoubleEdge),

    // ── Stat/crit combos ──────────────────────────────────────────────
    (ItemKind::LuckyCharm, ItemKind::LuckyHorseshoe, SynergyEffect::GuaranteedCrit),
    (ItemKind::VampireFangs, ItemKind::RustyNails, SynergyEffect::BloodLifesteal),

    // ── Utility / defense combos ──────────────────────────────────────
    (ItemKind::EnergyShield, ItemKind::Bandage, SynergyEffect::OverShield),
    (ItemKind::Featherlight, ItemKind::SteelToe, SynergyEffect::SpeedShoot),
    (ItemKind::CursedRing, ItemKind::Bandage, SynergyEffect::RegenerativeArmor),

    // ── Scaling / weapon combos ───────────────────────────────────────
    (ItemKind::MagnifyingGlass, ItemKind::LuckyCharm, SynergyEffect::BeamWide),
    (ItemKind::CursedRing, ItemKind::ExplosiveCharge, SynergyEffect::BigBoom),
    (ItemKind::GlassCannon, ItemKind::CursedRing, SynergyEffect::CriticalCurse),
];

/// Return a human-readable name for a synergy effect.
pub fn synergy_name(effect: SynergyEffect) -> &'static str {
    match effect {
        SynergyEffect::SuperPierce => "SUPER PIERCE",
        SynergyEffect::GuaranteedCrit => "GUARANTEED CRIT",
        SynergyEffect::BloodLifesteal => "BLOOD LIFESTEAL",
        SynergyEffect::EchoChain => "ECHO CHAIN",
        SynergyEffect::ExplosiveRetaliation => "EXPLOSIVE RETALIATION",
        SynergyEffect::DoubleEdge => "DOUBLE EDGE",
        SynergyEffect::SpeedShoot => "SPEED SHOOT",
        SynergyEffect::OverShield => "OVER SHIELD",
        SynergyEffect::RegenerativeArmor => "REGENERATIVE ARMOR",
        SynergyEffect::BeamWide => "BEAM WIDE",
        SynergyEffect::BigBoom => "BIG BOOM",
        SynergyEffect::CriticalCurse => "CRITICAL CURSE",
    }
}

/// Check which new synergies activate when `new_item` is added to
/// `collected_items`. Returns the list of newly-activated effects.
pub fn check_new_synergies(
    new_item: ItemKind,
    collected_items: &[ItemKind],
    active_synergies: &[SynergyEffect],
) -> Vec<SynergyEffect> {
    let mut new = Vec::new();
    for &(a, b, effect) in SYNERGIES {
        // Does this synergy involve `new_item`?
        if a != new_item && b != new_item {
            continue;
        }
        let other = if a == new_item { b } else { a };
        // Do we already have the other item?
        if !collected_items.contains(&other) {
            continue;
        }
        // Is the synergy already active?
        if active_synergies.contains(&effect) {
            continue;
        }
        new.push(effect);
    }
    new
}