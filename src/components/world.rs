//! Room, door, wall, spawner, and floor-transition components — the spatial
//! and progression data model for the Megabonk-style room system.

use bevy::prelude::*;

use super::enemy::EnemyArchetype;
use crate::components::PickupKind;

// ─── Biome enum ────────────────────────────────────────────────────────────

/// Per-floor biome theme that drives wall colors and enemy composition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Biome {
    #[default]
    Catacombs, // Floor 0: dark gray/purple, undead theme
    Frost,     // Floor 1: ice blue, fast enemies
    Magma,     // Floor 2: deep red/orange, heavy enemies
    Verdant,   // Floor 3: green, swarm enemies
    Void,      // Floor 4+: near-black, all enemies elite+
}

/// Map a floor number to its biome.
pub fn biome_from_floor(floor: u32) -> Biome {
    match floor {
        0 => Biome::Catacombs,
        1 => Biome::Frost,
        2 => Biome::Magma,
        3 => Biome::Verdant,
        _ => Biome::Void,
    }
}

// ─── Room system (Phase 2) ────────────────────────────────────────────────

/// Unique identifier for a room in a biome. `u8` gives 256 rooms
/// per biome — more than enough for Phase 2's 3 rooms and the
/// Phase 3+ multi-biome scaling.
///
/// Newtype (not a bare `u8`) so the type system catches "you
/// passed an enemy archetype id where a room id was expected"
/// at compile time. `#[derive(Default)]` defaults to `RoomId(0)`
/// so a freshly-spawned enemy / projectile etc. has a
/// placeholder room id (it'll be overwritten by the spawner
/// or filtered out by `RoomScope`-aware systems).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct RoomId(pub u8);

/// A rectangular room in a biome. The player can move freely
/// inside the `bounds` rectangle. Walls sit on the rectangle's
/// edges; doors are triggers on the floor in front of the walls.
///
/// `bounds_min` and `bounds_max` are the world-space corners.
/// The room is the closed set `[min, max]` on each axis. Walls
/// sit just outside the rectangle (offset by
/// `RoomsPlugin::WALL_THICKNESS` / 2) so the AABB is
/// `bounds_max + thickness` in extent.
#[derive(Clone, Copy, Debug)]
pub struct Room {
    pub id: RoomId,
    pub kind: RoomKind,
    pub bounds_min: Vec2,
    pub bounds_max: Vec2,
}

impl Room {
    pub fn center(&self) -> Vec2 {
        (self.bounds_min + self.bounds_max) * 0.5
    }
    pub fn size(&self) -> Vec2 {
        self.bounds_max - self.bounds_min
    }
}

/// Semantic role of a room. Phase 2 ships 3 rooms (Spawn /
/// Fight / Extract); Patch 6 will add the "Extract" exit portal
/// inside the Extract room. New room kinds (Treasure, Gauntlet
/// per PLAN.md §6) slot in here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomKind {
    /// Where the player spawns at the start of a run. No
    /// enemies, no pickups. The "calm before the storm".
    Spawn,
    /// Where the bulk of combat happens. Spawners are
    /// scoped to this room (Phase 2 minimum: all 4 spawners
    /// in the Fight room).
    Fight,
    /// Boss room. Every N rooms (default 4) the player faces
    /// a boss encounter. Boss death → full heal + floor
    /// advancement + difficulty ramp.
    Boss,
    /// Mini-boss room. Appears between boss rooms (every 2nd
    /// non-boss room). Tougher waves, more breakables, better
    /// drops. A mid-cycle spike before the next boss.
    MiniBoss,
    /// Horde room. Many weak enemies, fast intervals, bullet-heaven
    /// style. Tilted toward melee archetypes (chaser, dasher).
    /// More breakables for extra loot.
    Horde,
    /// Shop room. No enemies — the player walks in, buys upgrades
    /// with gold, then leaves via an exit pad toward the boss or
    /// next combat room. Always appears one room before the boss
    /// (cycle position 5 → boss at position 0).
    Shop,
    /// Treasure room: displays 2-3 items on pedestals. Player can
    /// take ONE for free. No enemies.
    Treasure,
    /// Challenge room: locked arena with 3 waves. Clear all waves
    /// to earn a weapon + gold reward.
    Challenge,
    /// Secret room: hidden behind a wall segment. Small room with
    /// free loot (gold, items, heal). No enemies.
    #[allow(dead_code)]
    Secret,
}

/// Door trigger: a small AABB on the floor in front of a wall.
/// When the player's AABB overlaps a door,
/// `door_transition_system` teleports them to `to_room` at
/// `entry_position` and despawns the old room's transient
/// entities (enemies, projectiles, puffs, pickups).
///
/// Doors don't have a collider of their own that blocks the
/// player — the surrounding wall does that. The door's
/// `Collider` is just the AABB used for the overlap check.
/// (Walls are full-perimeter; doors are floor triggers on
/// the inside of the room, not gaps in the walls.)
///
/// Phase 2.3: added `from_room`, `locked`, and `forward`.
/// - `from_room`: the room the player is IN when they step
///   on this door's trigger. The "source" of the transition.
///   Symmetric to `to_room` (the destination). Without this
///   field, `door_unlock_system` would have to infer the
///   source from the door's position relative to a room,
///   which is brittle and breaks for branching paths in
///   Phase 3+.
/// - `locked = true` means the door refuses to fire the
///   transition even if the player overlaps its AABB.
///   Set to `false` by `door_unlock_system` when the
///   source room's waves are cleared.
/// - `forward = true` marks this door as a "progression"
///   door — it's locked by default and unlocks on clear.
///   Backward doors (`forward = false`) are never locked:
///   the player can always retreat to a previous room.
#[derive(Component, Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct Door {
    pub from_room: RoomId,
    pub to_room: RoomId,
    pub entry_position: Vec2,
    pub locked: bool,
    pub forward: bool,
}

/// Marker on entities that belong to a specific room. Used
/// by `spawner_tick_system` to filter which spawners tick
/// (only the ones in the current room) and by future per-
/// room scoping systems (e.g. per-room decoration lights,
/// per-room audio cues).
///
/// Phase 2 attaches `RoomScope` only to spawner entities
/// (one per archetype, all in the Fight room). Enemies
/// don't need it — they're spawned in the current room and
/// despawned on transition.
#[derive(Component, Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct RoomScope {
    pub room_id: RoomId,
}

/// Static wall entity. Walls block player + enemy movement
/// (but not projectiles, which fly through freely in Patch 2).
/// Has `Transform + Sprite + Collider` like other colliders,
/// plus this marker so `physics_step_system` and the door
/// system can find them.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Wall;

/// Floor transition text that appears briefly when the player
/// advances to a new floor (after clearing a boss room).
/// The timer starts when the text is spawned; the fade system
/// ticks it, fades the alpha, and despawns when it finishes.
#[derive(Component)]
pub struct FloorTransitionText(pub Timer);

/// Phase 2 P6 (Crab Champions run): teleport pad that appears
/// in a cleared room. The player walks into it to transition
/// to the next room. Label describes the destination kind
/// (e.g. "Fight", "Boss") for future UI display.
/// Spawned by `pad_spawn_on_clear_system` in rooms.rs.
#[derive(Component, Clone, Debug)]
pub struct TeleportPad {
    pub label: String,
}

// ─── More-room types (Treasure, Challenge, Secret) ──────────────────────

/// Marker on a treasure pedestal entity. Each pedestal holds an item;
/// when the player walks over it, they receive the item and the pedestal
/// (and all remaining pedestals) despawn after a delay.
#[derive(Component, Clone, Debug)]
pub struct TreasurePedestal {
    #[allow(dead_code)]
    pub item: PickupKind,
    #[allow(dead_code)]
    pub taken: bool,
}

/// Tracked per challenge room: which wave is active and whether enemies
/// have been spawned for the current wave. The challenge system spawns
/// enemies directly rather than using the director-based WaveSpawner.
#[derive(Component, Clone, Debug)]
pub struct ChallengeState {
    #[allow(dead_code)]
    pub wave: u32,
    #[allow(dead_code)]
    pub enemies_spawned: bool,
}

/// Entity id of the last wave's elite enemy (if any). Used by the
/// challenge clear system to gate reward on wave 3 elite death.
#[derive(Component, Clone, Debug)]
pub struct ChallengeElite {
    pub elite_entity: Entity,
}

/// Marker on wall entities that block the exit doors during a challenge
/// room. Despawned when the challenge is completed.
#[derive(Component, Clone, Copy, Debug)]
pub struct ChallengeWall;

/// Marker on wall entities that serve as hidden secret-room entrances.
/// Transparency toggles when the player walks near them.
#[derive(Component, Clone, Debug)]
#[allow(dead_code)]
pub struct SecretWall {
    pub revealed: bool,
    pub room_id: RoomId,
}

/// Resource tracking an active challenge run in the current room.
#[derive(Resource, Clone, Debug)]
#[allow(dead_code)]
pub struct ChallengeRun {
    pub wave: u32,
    pub room_entity: Entity,
}

// ─── Wave spawning (Phase 2.3) ────────────────────────────────────────────

/// One wave in a room's progression: spawn `count` enemies of
/// `archetype`, spaced `interval_s` apart.
///
/// A wave is "done" when `count` enemies have been spawned.
/// The `WaveSpawner` then advances to the next wave in its
/// `waves` vec. When all waves are done, the room is "cleared"
/// and its forward door(s) unlock.
///
/// `interval_s = 0.0` means "spawn all enemies of this wave
/// on the same frame" — useful for cinematic openings or
/// scripted "ambush" waves. The `wave_spawner_tick_system`
/// treats 0.0 as a special case (spawn all remaining, then
/// advance immediately).
#[derive(Clone, Copy, Debug)]
pub struct WaveDefinition {
    #[allow(dead_code)]
    pub archetype: EnemyArchetype,
    #[allow(dead_code)]
    pub count: u32,
    #[allow(dead_code)]
    pub interval_s: f32,
}

/// One wave spawner per room that has enemies. Replaces the
/// Phase 1 / 2.0 design of 4 parallel `Spawner` entities
/// (one per archetype, all firing every 3s in parallel).
///
/// State machine:
/// 1. `current_wave < waves.len()` → ticking `wave_timer`
/// 2. When the timer fires and `spawned_this_wave < waves[current_wave].count`,
///    spawn one enemy and reset the timer to `interval_s`.
/// 3. When `spawned_this_wave == waves[current_wave].count`, advance:
///    `current_wave += 1`, `spawned_this_wave = 0`, re-arm `wave_timer`
///    with the NEXT wave's `interval_s`.
/// 4. When `current_wave == waves.len()` (all waves done), the
///    spawner stops ticking. `door_unlock_system` observes this
///    and sets `locked = false` on the room's forward door(s).
///
/// `room_id` replaces the Phase 2.0 `RoomScope` marker for
/// spawners — the spawner is now a struct with its own room
/// field, so it doesn't need a separate component.
///
/// `wave_timer` is `TimerMode::Repeating`-equivalent: each
/// spawn resets it to the current wave's `interval_s` (so
/// the next spawn is `interval_s` later, not on the next
/// frame). This matches the Phase 1 `Spawner.interval`
/// semantics.
#[derive(Component, Debug)]
pub struct WaveSpawner {
    pub room_id: RoomId,
    pub waves: Vec<WaveDefinition>,
    pub current_wave: usize,
    pub spawned_this_wave: u32,
    pub wave_timer: Timer,
    /// Farming phase spawn interval in seconds. Set per room type
    /// in generate_room. Lower = faster spawns. Default 0.35.
    pub farming_interval: f32,
    /// Seconds elapsed since the farming phase started. Used for a
    /// universal speed ramp that applies to ALL rooms — the longer
    /// you stay in a room, the faster enemies spawn.
    pub farming_elapsed: f32,
    /// Seconds before RoomClearedEvent fires for non-boss rooms.
    /// Counts down each farming tick. When it reaches 0, the room
    /// is cleared and teleport pads appear. Boss rooms set this to
    /// 0 (boss death gates clear instead).
    pub clear_delay: f32,
    /// Phase 3b (RoR2 director budget): accumulated director
    /// credits for buying enemies. Accumulates at a per-room-type
    /// rate each frame; spent when an enemy is spawned. Starts at
    /// 8.0 so the first few spawns happen immediately.
    pub director_credits: f32,
}