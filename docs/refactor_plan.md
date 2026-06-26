# RUSTWORN — Full Refactoring Plan

**Goal:** Split every file >500 lines into small modules, clean up 0-byte stubs, and establish a clean module structure.

**Architecture:** One module per concern — components, plugins, and resources each get their own directory with sub-modules.

**Current state:** 22 source files. 7 files >500 lines (monoliths). 4 0-byte stubs from a previous aborted refactor.

---

## Phase 0: Delete the stubs

### Task 1: Remove 0-byte stubs

These files were created by the previous refactor but never filled. They cause `E0428` (duplicate name) because `mod.rs` already has the code they were *supposed* to hold.

**Files to delete:**
- `src/plugins/enemy/boss.rs` — 0 bytes
- `src/plugins/enemy/death_aoe.rs` — 0 bytes
- `src/plugins/enemy/contact.rs` — 0 bytes

These are dead files. `mod.rs` doesn't reference them as `pub mod` — they're just orphans.

### Task 2: Clean `ai.rs` + `ai.rs.new`

`ai.rs` is 108 lines (content), `ai.rs.new` is 622 lines. The `.new` is the actual replacement. Move it.

### Task 3: Move `spawn1.rs` → merge into `spawn.rs`

`spawn1.rs` (40 lines) was split from `spawn.rs` (668 lines). Work out what it is and merge.

---

## Phase 1: Split `components.rs` (1073 → 118 types)

`components.rs` has 118 component/resource types. Split into:

| New file | Source | Line target |
|---|---|---|
| `src/components/mod.rs` | `pub mod` re-exports | ~10 |
| `src/components/player.rs` | `Player`, `Weapon`, `Projectile`, `Team`, `Health`, `Collider` | ~150 |
| `src/components/enemy.rs` | `Enemy`, `AiBehavior`, `EnemyArchetype`, state types | ~200 |
| `src/components/world.rs` | `Room`, `RoomKind`, `Door`, `Biome`, `RoomBounds` | ~150 |
| `src/components/misc.rs` | `Pickup`, `PickupKind`, `Shrine`, `RunState` fields | ~100 |
| `src/components/ui.rs` | HUD types | ~80 |

## Phase 2: Split `enemy/mod.rs` (2012 lines)

This is the real monolith. Split into:

| New file | Current lines | Content |
|---|---|---|
| `src/plugins/enemy/spawn.rs` | 668 (already exists) | Spawn helpers, `spawn_chaser`, `spawn_dasher`, etc. |
| `src/plugins/enemy/ai.rs` | 108 (needs filling) | `enemy_ai_system` — the big `match AiBehavior` |
| `src/plugins/enemy/waves.rs` | 229 (already exists) | `WaveSpawner`, `director_pick` |
| `src/plugins/enemy/mod.rs` | ~30 (was 2012) | Just `pub mod` re-exports + `EnemyPlugin` |

## Phase 3: Split `combat.rs` (1317 lines)

| New file | Source | Content |
|---|---|---|
| `src/plugins/combat/mod.rs` | re-exports | ~10 |
| `src/plugins/combat/weapon.rs` | from ~line 0-400 | `weapon_fire_system`, `weapon_cooldown` |
| `src/plugins/combat/projectile.rs` | from ~400-800 | `projectile_lifetime`, `projectile_damage` |
| `src/plugins/combat/damage.rs` | from ~800-1317 | `damage_system`, `enemy_contact` |

## Phase 4: Split `rooms.rs` (1138 lines)

| New file | Content |
|---|---|
| `rooms/generate.rs` | `generate_room`, `room_kind` |
| `rooms/pads.rs` | teleport pads, pad_spawn_on_clear |
| `rooms/shop.rs` | shop items, collect |
| `rooms/transitions.rs` | teleport/door transitions |

## Phase 5: Split `hud.rs` (806 lines)

| New file | Content |
|---|---|
| `hud/bars.rs` | HP bars, weapon wheels |
| `hud/timer.rs` | Cascade timer, clear timer |
| `hud/overlay.rs` | Screen effects, transition overlays |

## Phase 6: Split `resources.rs` (794 lines)

| New file | Content |
|---|---|
| `src/resources/mod.rs` | re-exports |
| `src/resources/state.rs` | `RunState`, `RoomState`, `FloorState` |
| `src/resources/tuning.rs` | `GameSettings` |
| `src/resources/rng.rs` | `RunRng` |
| `src/resources/hitstop.rs` | `Hitstop`, `ScreenShake` |

## Phase 7: Split `player.rs` (686 lines)

| New file | Content |
|---|---|
| `player/movement.rs` | input/movement |
| `player/dash.rs` | dash/slide |
| `player/state.rs` | game state transitions, spawn |

---

## Module re-export pattern

Each module directory follows:

```rust
// src/<module>/mod.rs
pub mod sub_a;
pub mod sub_b;
pub mod sub_c;

pub use sub_a::Thing;
pub use sub_b::Other;

// Plugin struct in mod.rs
pub struct SomePlugin;
impl Plugin for SomePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (...));
    }
}
```

The `plugins/mod.rs` just gets `pub mod` declarations — no change.

---

## Rules

1. **No logic changes** — only move/cut/paste code. Logic stays identical.
2. **`pub use`** every extracted type, function, and system so `lib.rs` doesn't change.
3. **Commit after every 3 files** — git keeps history clean.
4. **`cargo check` after every phase** — never leave broken builds.
5. **Delete** the 0-byte orphans in Phase 0.
6. **`ai.rs.new`** → rename to `ai.rs` (it's the actual content).

---

## Effort: ~6-8h of mechanical cut work

Phase 0 (stub cleanup) is 5 minutes. The real work starts at Phase 1.