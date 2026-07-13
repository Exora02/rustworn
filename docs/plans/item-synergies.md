# Item Synergies — Isaac-Style Build Depth

## Design
When a player holds two specific items, they get a bonus effect.
Synergies are discovered naturally — the game doesn't tell you upfront.
12 items → ~66 possible pairs; I'll design ~20 that are interesting.

### Synergy Table
Pairs with meaningful interactions:

| Item A | Item B | Synergy Effect |
|--------|--------|---------------|
| SharpenedEdge | TitaniumPlating | +1 pierce AND armor = projectiles pierce ALL enemies (ignore armor check) |
| RabbitsFoot | LuckyHorseshoe | Luck +0.3 = every 3rd hit is a guaranteed crit |
| Adrenaline | VampireFangs | At low HP, attacks lifesteal 3x instead of 2x damage |
| ConductingRod | EchoChamber | Chain lightning hits echo: each chained target triggers a second chain |
| ReactiveArmor | ExplosiveCharge | Getting hit triggers an explosion (not just shockwave) |
| BloodPrice | GlassCannon | Deal 2x damage but take damage on kill too (risk-reward) |
| HeavyMagazine | Scope | Attack speed bonus no longer slows movement |
| Glue (new item idea) | SharpenedEdge | Projectiles stick to enemies, dealing damage over time |
| SmokeBomb | RageMode | Activating rage cloaks you in smoke (invisible to enemies) |
| HealingAura | EnergyShield | Healing overfills shield (temporary overshield) |
| TimeWarp | Cascade | Time warp duration doubled |
| Featherlight | Dash | Dash leaves afterimages that deal damage |
| LuckyCharm | MagnifyingGlass | Luck increases beam width |
| SteelToe | MeleeWeapon | Melee attacks stun for 2x duration |
| Bandage | TitaniumPlating | Armor also heals 0.5 HP per room |
| VampireFangs | Lifesteal perk | Lifesteal also heals allies (future) |
| ExplosiveCharge | RocketLauncher | Rockets have 2x explosion radius |
| CursedRing | GlassCannon | Curse also increases crit damage (2x → 3x instead of penalty) |

### Synergy Data Structure
In src/components/misc.rs:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SynergyEffect {
    SuperPierce,           // Pierce ALL enemies
    GuaranteedCritCombo,   // Every 3rd hit auto-crits
    BloodLifesteal,        // 3x lifesteal at low HP
    EchoChain,             // Chains trigger secondary chains
    ExplosiveRetaliation,  // Getting hit = explosion
    DoubleEdge,            // 2x damage, damage on kill
    SpeedShoot,            // No move speed penalty from items
    OverShield,            // Healing overfills shields
    TimeDouble,            // Time warp duration x2
    DashAfterimage,        // Dash leaves damaging trails
    BeamWide,              // Luck increases beam width
    StunMaster,            // Melee stuns 2x
    RegenerativeArmor,     // Armor heals 1 HP per room
    BigBoom,               // 2x explosion radius
    CriticalCurse,         // Curse boosts crit damage
}

pub struct Synergy {
    pub item_a: ItemKind,
    pub item_b: ItemKind,
    pub effect: SynergyEffect,
}
```

### Where to wire
In src/plugins/pickups.rs, when a player collects an item:
- Check if the player already has any other items
- For each pair the player holds, check the synergy table
- If a synergy applies: apply the SynergyEffect to RunState (new field)

In src/plugins/combat/ (damage.rs, projectile.rs, etc.):
- Check run.active_synergies for relevant effects
- Modify damage/spawn behavior accordingly

### Implementation Plan
1. Add SynergyEffect enum + Synergy struct + synergy table to a new file or misc.rs
2. Add active_synergies: Vec<SynergyEffect> to RunState
3. In pickup handler: after applying item, check for new synergies
4. Wire specific effects into damage/projectile/melee systems
5. Add synergy popup text (gold color, same as item pickup text)