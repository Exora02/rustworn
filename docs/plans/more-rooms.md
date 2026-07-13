# More Room Types

## Design
Add 3 new room types to increase variety and discovery:

### 1. Secret Room
- Hidden behind walls, discovered by walking near them
- +10% chance per floor to spawn (hidden behind a random wall segment)
- Contains: gold, a free item, or a chest
- Player walks near it → wall becomes slightly transparent → can walk through
- Marker: RoomKind::Secret

### 2. Treasure Room
- A room with 2-3 items displayed on pedestals
- Player can take ONE for free
- Items are from the existing item pool
- Marker: RoomKind::Treasure

### 3. Challenge Room
- When you enter, a gate locks behind you
- 3 waves of enemies spawn
- Clear all 3 waves → gate opens, reward drops
- Reward: random weapon + gold
- Marker: RoomKind::Challenge

### Implementation

1. Add to RoomKind enum (world.rs):
```rust
pub enum RoomKind {
    Spawn,
    Fight,
    Boss,
    MiniBoss,
    Horde,
    Shop,
    Treasure,
    Challenge,
    Secret,
}
```

2. In room generation (rooms/mod.rs or transitions.rs):
- When generating a room floor, replace some Fight rooms with new types
- Treasure: appears every 3rd room (replaces Fight)
- Challenge: appears every 5th room (replaces MiniBoss)
- Secret: spawned as a hidden room adjacent to any room

3. For Secret rooms:
- Walk near wall → spawn a secret_door_trigger zone
- Player overlaps trigger → wall entity becomes transparent
- Player walks through → enters secret room
- Secret room has 1-2 pickups and no enemies

4. For Treasure rooms:
- Spawn 2-3 colored pedestal entities
- Player walks over pedestal → spawn PickupKind::Item
- Pedestal disappears after collecting

5. For Challenge rooms:
- On entry: spawn wall entities blocking doors
- Spawn 3 waves (each wave = stronger enemies)
- Track challenge_progress: u32 (wave counter)
- After wave 3: walls despawn, reward drops
- Reward: PickupKind::Weapon + PickupKind::Gold