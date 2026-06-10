# P6 — Crab Champions Run Structure

> **Goal**: Replace the static 3-room layout with an infinite, dynamically generated room sequence. Doors → Teleport Pads that appear on room clear. Branching paths. Boss rooms every N rooms. Biome cycles.

---

## Architecture

**Current**: 3 hard-coded rooms (R0/R1/R2), door triggers, door_unlock_system, RoomClearedEvent → unlock forward door.

**New**:
- Static `setup_rooms_system` replaced by dynamic `FloorState` resource
- Rooms generated on-the-fly by `generate_room(floor: u32, kind: RoomKind)` 
- `Door` → `TeleportPad` component (spawned on room clear, player overlap → next room)
- `BiomeState` → `FloorState { current_room_id, rooms_cleared, floor_number, rooms: Vec<Room> }`
- `RoomClearedEvent` spawns TeleportPad instead of unlocking a door
- Boss every 4 rooms → Biome change → full heal + difficulty ramp

---

## Task 1: add TeleportPad component + floor-tracking resource

**Files:**
- Modify: `src/components.rs` — add `TeleportPad` marker component, tweak `RoomKind` (remove `Extract`, add `Boss`)
- Modify: `src/resources.rs` — replace `BiomeState` with `FloorState`

## Task 2: dynamic room generator

**Files:**
- Modify: `src/plugins/rooms.rs` — add `generate_room(floor, kind) → Room` function, replace `setup_rooms_system` to use it

## Task 3: teleport pad spawn on room clear

**Files:**
- Modify: `src/plugins/rooms.rs` — `door_unlock_system` → `pad_spawn_on_clear_system`, spawns `TeleportPad` in room center

## Task 4: teleport transition

**Files:**
- Modify: `src/plugins/rooms.rs` — `door_transition_system` → `teleport_transition_system`, player overlaps pad → generate next room, move player

## Task 5: boss rooms + biome cycle

**Files:** 
- Modify: `src/plugins/rooms.rs` — boss room every 4 clears, boss death → full heal + floor_number++
- Modify: `src/plugins/setup.rs` — `enter_in_run_system` resets `FloorState`
- Modify: `src/components.rs` — add `BiomeTransition` marker or event

## Task 6: cleanup obsolete code

**Files:**
- Remove: `Door` component (no longer used)
- Remove: spawn_doors, spawn_walls logic for old layout
- Tests: update to work with new system

---

Let me start executing Task 1.