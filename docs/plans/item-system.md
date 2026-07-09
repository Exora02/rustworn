# Item System — Brotato/Gungeon-style Run Items

## Design
Items are found in runs (from enemies, breakables, shop, secret rooms).
Each item modifies the expanded RunState stats or Weapon stats.

### Item Kind Pool (first batch — 12 items)
| Name | Effect | Rarity |
|------|--------|--------|
| Rusty Nails | +0.5 base_damage | Common |
| Lucky Charm | +5% crit_chance | Common |
| Featherlight | +10% move_speed_mult | Common |
| Steel Toe | +1 pierce | Common |
| Bandage | +0.5 hp_regen_per_sec | Common |
| Magnifying Glass | +10% pickup_range | Common |
| Glass Cannon | +50% damage, -30% max HP | Uncommon |
| Vampire Fangs | +5% lifesteal_pct | Uncommon |
| Explosive Charge | +20 explosion_radius | Uncommon |
| Energy Shield | +1.0 shield | Uncommon |
| Lucky Horseshoe | +10% luck | Uncommon |
| Cursed Ring | +20% all stats, +1 curse | Rare |

### PickupKind variants
Add new variants to PickupKind:
```rust
Item(ItemKind),
```

Where ItemKind is:
```rust
pub enum ItemKind {
    RustyNails,
    LuckyCharm,
    Featherlight,
    SteelToe,
    Bandage,
    MagnifyingGlass,
    GlassCannon,
    VampireFangs,
    ExplosiveCharge,
    EnergyShield,
    LuckyHorseshoe,
    CursedRing,
}
```

### Drop system
Items drop from:
- Bosses: 100% chance to drop 1 item
- Elite enemies: 15% chance
- Regular enemies: 3% chance (scales with luck)
- Shop: items for gold

### Apply item effects
Item effects modify RunState stats directly. Hook into the expanded stats:
- RustyNails: run.base_damage += 0.5
- LuckyCharm: run.crit_chance = (run.crit_chance + 0.05).min(1.0)
- Featherlight: run.move_speed_mult += 0.1
- SteelToe: run.pierce += 1
- Bandage: run.hp_regen_per_sec += 0.5
- MagnifyingGlass: run.pickup_range += 10.0
- GlassCannon: run.base_damage *= 1.5; health.max *= 0.7
- VampireFangs: run.lifesteal_pct += 0.05
- ExplosiveCharge: run.explosion_radius += 20.0
- EnergyShield: run.shield += 1.0
- LuckyHorseshoe: run.luck += 0.1
- CursedRing: all stats * 1.2, run.curse += 1

### Pickup visual
Items use a colored sprite with a star icon (or just a colored square):
- Common: gray
- Uncommon: green
- Rare: gold

## Files to modify
- `src/components/misc.rs` — ItemKind enum, new PickupKind::Item variant
- `src/components/mod.rs` — re-export ItemKind
- `src/plugins/pickups.rs` — apply_pickup match arm for Item(kind), drop system
- `src/plugins/hud/juice.rs` — push_pickup_popup match for ItemKind
- `src/plugins/enemy/spawn.rs` — boss drop items
- `src/plugins/combat/damage.rs` / projectile.rs — elite/regular drop items