# Multi-Biome Visuals Implementation Plan

**Goal:** Each floor has a distinct visual theme + enemy composition. Walls, pads, shrines, and enemies tint/change per floor.

## Already in place
- `FLOOR_COLORS` in `rooms/mod.rs` — 5 color stops used for wall rendering
- `biome_index` in `RunState` — tracks current floor number

## What to add

### 1. Biome enum + descriptions
Add to `src/components/world.rs`:
```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Biome {
    #[default]
    Catacombs,   // Floor 0: dark gray/purple, undead theme
    Frost,       // Floor 1: ice blue, fast enemies
    Magma,       // Floor 2: deep red/orange, heavy enemies
    Verdant,     // Floor 3: green, swarm enemies
    Void,        // Floor 4+: near-black, all enemies elite+
}
```

Add `biome_colors()` fn that returns (wall_color, accent_color, floor_glow).

### 2. Biome-specific enemy spawn weights
Modify `src/plugins/enemy/waves.rs` (where `WaveSpawner` uses credits to spawn enemies):
- Add a lookup that returns different cost weights per biome
- Catacombs: balanced mix
- Frost: more dashers + shooters (fast, ranged)
- Magma: more bombers + chasers (explosive, heavy)
- Verdant: more orbiter + chaser swarms
- Void: all elite variants, higher HP

### 3. Visual changes per biome
- Wall tint already uses FLOOR_COLORS ✓
- New: tint enemy sprites with a biome accent overlay
- New: change pad colors per biome (e.g., Frost pads are icy blue)
- New: shrine colors shift per biome

### 4. Floor transition notification
When entering a new biome (floor 0→1, 1→2, etc.), show a brief centered text:
"--- CATACOMBS ---" / "--- FROST REACH ---" etc.
Reuse the existing FloorTransitionText component pattern.

## Files to modify
- `src/components/world.rs` — Biome enum
- `src/components/mod.rs` — re-export
- `src/plugins/rooms/mod.rs` — FLOOR_COLORS stays, add accent colors per biome
- `src/plugins/rooms/generate.rs` — use biome accent for wall glow/color
- `src/plugins/enemy/waves.rs` — biome-specific spawn weights
- `src/plugins/setup.rs` — biome info in HUD title
- `src/resources/state.rs` — biome name string for HUD