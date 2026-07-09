# Boss Variety Per Biome

**Goal:** Each biome has a unique boss with different visual, stats, and attack pattern.

## Current state
Bosses are spawned by `spawn_boss` in `src/plugins/enemy/spawn.rs`. The boss is a single large sprite with no special attacks — just sits there and takes damage.

## Design

| Biome | Boss Name | Visual | Behavior | Special |
|-------|-----------|--------|----------|---------|
| Catacombs (0) | **Lich** | Purple, 60px | Slow, fires projectiles in bursts | Summons 3 chasers on death |
| Frost (1) | **Yeti** | Ice blue, 80px | Charges at player periodically | Leaves ice patches that slow |
| Magma (2) | **Inferno** | Orange/red, 70px | Lobs explosive projectiles | AoE ground pound |
| Verdant (3) | **Treant** | Green, 90px | Spawns orbs that orbit | Heals periodically |
| Void (4+) | **Void Walker** | Near-black with purple glow, 75px | Teleports around room | Fires homing projectiles |

## Implementation

### 1. Boss phase system (already exists)
`src/components/enemy.rs` already has `BossPhase` enum. Extend it:
```rust
pub enum BossPhase {
    Idle,
    Attacking,
    Summoning,  // New: spawning minions
    Charging,   // New: wind-up for charge attack
    Teleporting, // New: void walker teleport
}
```

### 2. Per-biome boss spawning
Modify `src/plugins/enemy/spawn.rs`:
- Accept a `Biome` parameter in `spawn_boss`
- Return a different boss entity with different stats per biome
- Boss size, color, HP, damage, and behavior all vary

### 3. Boss attack system
Create a new file `src/plugins/enemy/boss_attacks.rs` with systems that read the boss entity + its phase:
- Lich: every 3s, fire 3 projectiles in a spread pattern toward player
- Yeti: every 5s, charge at player position (high speed dash toward player)
- Inferno: every 4s, lob a projectile that lands at player position as AOE
- Treant: every 6s, heal for 2 HP, spawn 2 orbiting projectiles
- Void Walker: every 3s, teleport to a random position, fire homing projectile toward player