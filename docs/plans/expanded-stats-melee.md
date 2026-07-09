# Expanded Stats + Item System Foundation (Brotato-style)

## Current stats on RunState:
- damage_stacks: u32
- fire_rate_stacks: u32
- speed_stacks: u32
- regen_per_sec: f32
- curse: u32
- gold: u32
- armor: f32
- gold_mult: f32
- vampire_heal: f32
- thorns_damage: f32

## New stats to add to RunState:

### Offensive
- `base_damage: f32 = 1.0` — raw weapon damage multiplier
- `range_mult: f32 = 1.0` — projectile range / beam range
- `crit_chance: f32 = 0.0` — flat crit rate (0-1)
- `crit_mult: f32 = 2.0` — crit damage multiplier
- `pierce: u32 = 0` — projectiles pierce this many enemies
- `explosion_radius: f32 = 0.0` — projectiles explode on hit
- `lifesteal_pct: f32 = 0.0` — % of damage dealt healed

### Defensive
- `dodge_chance: f32 = 0.0` — flat dodge rate (0-1)
- `hp_regen_per_sec: f32 = 0.0` — separate from the pickup-stack regen
- `damage_reduction: f32 = 0.0` — flat % damage reduction
- `shield: f32 = 0.0` — extra HP layer that regens after 5s of no damage

### Utility
- `pickup_range: f32 = 40.0` — auto-pickup radius
- `move_speed_mult: f32 = 1.0` — separate multiplier for items
- `attack_speed_mult: f32 = 1.0` — fire rate multiplier
- `luck: f32 = 0.0` — affects drop rates and room quality
- `rerolls: u32 = 0` — free shop rerolls per run

## Melee Combat (Megabonk)

### MeleeWeapon component
```rust
#[derive(Component)]
pub struct MeleeWeapon {
    pub damage: f32,
    pub range: f32,       // swing arc radius
    pub arc_deg: f32,     // swing angle
    pub knockback: f32,
    pub cooldown: Timer,
}
```

### Melee attack system
- On left-click while holding a melee weapon (vs ranged):
  - Spawn a temporary collision zone in front of player
  - Apply damage + knockback to enemies in arc
  - Visual: temporary colored arc (sprite or line)

### Weapon switch
- Add `Melee` variant to WeaponArchetype
- Or make it a separate slot (press Q to melee)

### Weapon stats for item system
Add to Weapon component:
- `status_chance_mult: f32 = 1.0`
- `area_mult: f32 = 1.0`
- `projectile_count_mult: f32 = 1.0`

## Horde Density

In `src/plugins/enemy/waves.rs`:
- Increase `director_credits` (spawn budget) by biome index
- Add a room density multiplier based on curse
- Speed up spawn interval per cleared room
- Floor 4+ should feel like a VS horde (30+ enemies on screen)