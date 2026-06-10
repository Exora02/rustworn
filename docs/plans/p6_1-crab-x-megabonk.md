# P6.1 — Crab Champions x Megabonk: Room Interactables + Branching

> Two additions to make rooms feel alive: (1) shrines and breakables
> spawned in rooms, (2) branching teleport paths after clearing.

---

## Architecture

**Shrines**: Static entities in rooms that the player touches for a
one-time effect (heal, buff, curse). Spawned in `generate_room()`.
`shrine_collect_system` checks player overlap and applies effect.

**Breakables**: Fragile pots/crates that drop loot on destruction.
Simple HP component + drop on death. Projectiles hit them, beam hits
them. `breakable_damage_system` in combat.rs or rooms.rs.

**Branching teleport pads**: `pad_spawn_on_clear_system` currently
spawns 1 pad. Change to spawn 2-3 with different labels (Fight /
Treasure / Boss).

---

## Task 1: Shrines + Breakables components

**Files:**
- `src/components.rs` — add `Shrine`, `ShrineKind`, `Breakable`

## Task 2: Spawn interactables in rooms

**Files:**
- `src/plugins/rooms.rs` — add `spawn_shrines()`, `spawn_breakables()`
  to `generate_room()`
- `src/plugins/rooms.rs` — add `shrine_collect_system`
- `src/plugins/combat.rs` — add breakable damage handling

## Task 3: Branching teleport pads

**Files:**
- `src/plugins/rooms.rs` — `pad_spawn_on_clear_system` spawns 2-3
  pads instead of 1
- `teleport_transition_system` already handles label → room kind

---

Let me start implementing.