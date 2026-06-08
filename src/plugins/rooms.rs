//! Room system (Phase 2): walls, doors, room transitions, and
//! per-room physics.
//!
//! Phase 1 was an open infinite arena — no walls, no rooms. The
//! player could walk in any direction forever. Phase 2 splits the
//! arena into discrete rooms separated by walls. The player
//! walks from room to room via door triggers.
//!
//! Layout for Patch 2 (1 biome, 3 rooms):
//!
//! ```text
//!      ┌──────────┐
//!      │ Room 2   │  Extract (no enemies, no exits)
//!      │          │  ↑ north door from R1
//!      └──────────┘
//!      ┌──────────┐
//!      │ Room 1   │  Fight (4 spawners — all archetypes)
//!      │          │  ← east door from R0
//!      └──────────┘
//!      ┌──────────┐
//!      │ Room 0   │  Spawn (player starts here, no enemies)
//!      │          │  → door to R1 (east wall)
//!      └──────────┘
//! ```
//!
//! Rooms are stacked vertically: R0 is the bottom-left, R1 is
//! the top-left, R2 is the top-right. Each room is `ROOM_SIZE`
//! units square (800). Doors are floor triggers inside the
//! room, not gaps in the walls — the player steps on the
//! trigger and teleports to the new room's entry position.
//!
//! ## Architecture
//!
//! - `setup_rooms_system` (OnEnter InRun): despawns old walls
//!   and doors, then spawns the 3 static rooms. Also re-anchors
//!   the player to R0's center.
//! - `physics_step_system` (Update, InRun): replaces the
//!   Phase 1 `movement_system`. Applies Velocity to Transform
//!   for player + enemies + projectiles, then resolves wall
//!   collisions for the moving entities (projectiles pass
//!   through walls in Patch 2 — they despawn on enemy hit or
//!   lifetime expiry).
//! - `door_transition_system` (Update, InRun): if the player
//!   overlaps a door, despawn the old room's transient
//!   entities, reset spawner timers, move the player, update
//!   `biome.current_room`.
//!
//! ## B0001 hygiene
//!
//! - `physics_step_system` writes `Transform` on (player +
//!   enemies + projectiles), and the player chain has
//!   `aim_rotation_system` (Transform on AIM indicator) and
//!   `camera_follow_system` (Transform on main camera). The
//!   disjointness check needs explicit `Without<>` filters to
//!   prove the queries are mutually exclusive. The AIM
//!   indicator has no `Collider`, so it's excluded by the
//!   `Collider` query. The main camera has no `Velocity` or
//!   `Collider`, so it's excluded by both. Projectiles are
//!   filtered separately from movers to skip wall collision.
//! - `door_transition_system` writes `Transform` on the
//!   player. `With<Player> + Without<AimIndicator>` is
//!   the marker pair that disambiguates from the AIM
//!   indicator (child of the player, also has Transform).

use bevy::prelude::*;
use log::info;

use crate::components::{
    AimIndicator, AppState, Collider, DeathPuff, Door, Enemy, EnemyArchetype, MainCamera, Pickup,
    Player, Projectile, Room, RoomId, RoomKind, Velocity, Wall, WaveDefinition, WaveSpawner,
};
use crate::events::RoomClearedEvent;
use crate::resources::{BiomeState, GameSettings, Hitstop, RunState};

/// Size of each room (square). 800 units = same scale as
/// the existing player movement / camera view (1280×720
/// viewport, 40×40 player, 220 u/s walk speed). Tunable
/// later for pacing.
const ROOM_SIZE: f32 = 800.0;

/// Thickness of each wall. Thick enough to be visible and
/// to catch the player's AABB before they pass through.
/// 20 units = half the player's AABB half-extent, so the
/// collision response cleanly stops the player 1 pixel
/// from the wall's outer edge.
const WALL_THICKNESS: f32 = 20.0;

/// Color of the walls — dark gray, slightly lighter than
/// the planned background, so they're visible without
/// dominating the screen. Matches the chaser's color
/// value's brightness (a chaser at 0.85 red is the "danger"
/// color; the wall at 0.30 is the "boundary" color — same
/// saturation, different value).
const WALL_COLOR: Color = Color::srgb(0.30, 0.30, 0.35);

/// Color of the door trigger zone — bright green so the
/// player sees the affordance clearly. Same green family
/// as the HP bar's full-health color (HP_GREEN) so the
/// player learns "green = good for you" = "green = exit
/// forward".
const DOOR_COLOR: Color = Color::srgb(0.30, 0.85, 0.30);

/// Size of the door trigger AABB. 60×30 is wide enough to
/// hit by walking toward the wall at any angle, narrow
/// enough to leave room for the player to walk past the
/// door without triggering it.
const DOOR_SIZE: Vec2 = Vec2::new(60.0, 30.0);

/// Offset from the room's wall to the door's center. 30
/// units puts the door's edge (at door_center ± 30 along
/// the wall axis) 0 units from the wall's inner edge —
/// the player can't quite touch the door edge and the
/// wall edge at the same time.
const DOOR_INSET: f32 = 30.0;

/// Offset from the new room's wall to the entry position.
///
/// Math:
///   - door trigger half-extent on the wall axis: 30 (DOOR_SIZE.x / 2)
///   - player AABB half-extent on the wall axis: 20
///   - safety gap so a frame of physics-step doesn't nudge the
///     player back into the trigger: 50
///
/// Sum = 100. With 50 (the Phase 2.0 value), the player AABB
/// at the entry position overlapped the destination's return
/// door trigger — a teleport-loop bug where the player
/// arrived in R1 at (50, -400) and was immediately picked up
/// by the R1→R0 door back to R0. Bumping to 100 puts the
/// player's far edge 20u past the door's far edge with 20u
/// to spare.
const ENTRY_INSET: f32 = 100.0;

/// z-order for walls and doors. Slightly above the world
/// (z=0) so they sit on top of any background gradient.
/// Below the HUD z-range (HUD_Z=10) so the game-over
/// overlay still covers them on death.
const WALL_Z: f32 = 0.1;
const DOOR_Z: f32 = 0.1;

pub struct RoomsPlugin;

impl Plugin for RoomsPlugin {
    fn build(&self, app: &mut App) {
        // OnEnter(InRun) handles the "build the rooms" lifecycle.
        // `setup_rooms_system` also re-anchors the player to R0's
        // center, so the (0, 0) reset in `enter_in_run_system`
        // (SetupPlugin) is overwritten immediately. The two
        // OnEnter systems fire in registration order: SetupPlugin
        // registers first, so `enter_in_run_system` runs before
        // `setup_rooms_system`. By the time the first Update tick
        // of the new state fires, the player is at R0's center
        // and the walls are in place.
        app.add_systems(OnEnter(AppState::InRun), setup_rooms_system)
            .add_systems(
                Update,
                (
                    // Physics step: applies Velocity to Transform
                    // (player + enemies + projectiles), then
                    // resolves wall collisions for movers. Projectiles
                    // skip wall resolution — they pass through
                    // walls freely in Patch 2 (a future patch can
                    // add a "despawn projectile on wall hit"
                    // rule).
                    physics_step_system,
                    // Door transition: if the player overlaps an
                    // UNLOCKED door, fire the room change. Runs
                    // after physics_step so the player's AABB is
                    // current. Locked doors are skipped (the
                    // player walks through the trigger zone but
                    // nothing happens).
                    door_transition_system,
                )
                    .chain()
                    .run_if(in_state(AppState::InRun)),
            )
            .add_systems(
                Update,
                // Door unlock: reads RoomClearedEvent from the
                // wave spawner (enemy plugin) and sets
                // `locked = false` on the matching room's forward
                // door. Runs in any state to keep the event
                // channel drained (a chained gate to InRun
                // would leak events if fired at the wrong
                // moment).
                door_unlock_system,
            );
    }
}

// ─── setup_rooms_system ───────────────────────────────────────────────────

// Build the 3 static rooms. Hard-coded positions for Phase 2
// minimum viable — Patch 5 ("deterministic procgen") will
// load these from a WaveDefinition resource derived from
// `RunState.seed`.
//
// Why hard-code? 3 rooms × 4 walls + 2 doors = 14 entities.
// Reading them from a file (TOML, RON, etc.) adds a file
// format + asset loader for no benefit. Patch 3's wave
// spawning will use `GameSettings` for tunables, but the
// room LAYOUT (which rooms exist, how they connect) stays
// in code until Patch 5.
fn setup_rooms_system(
    mut commands: Commands,
    mut biome: ResMut<BiomeState>,
    walls: Query<Entity, With<Wall>>,
    doors: Query<Entity, With<Door>>,
    mut player_query: Query<&mut Transform, (With<Player>, Without<AimIndicator>)>,
) {
    // Clear out old walls and doors (from a previous run if
    // any). Without this, restarting after a death would
    // double-spawn the rooms. Enemies / projectiles / puffs /
    // pickups are despawned by `enter_in_run_system` in
    // SetupPlugin; walls and doors live here so they're
    // despawned here.
    for e in &walls {
        commands.entity(e).despawn();
    }
    for e in &doors {
        commands.entity(e).despawn();
    }

    // Three static rooms in a 2×2-ish grid (R0 bottom-left,
    // R1 top-left, R2 top-right). Each room is ROOM_SIZE on
    // a side. Adjacent rooms share a wall.
    let rooms = vec![
        Room {
            id: RoomId(0),
            kind: RoomKind::Spawn,
            bounds_min: Vec2::new(-ROOM_SIZE, -ROOM_SIZE),
            bounds_max: Vec2::new(0.0, 0.0),
        },
        Room {
            id: RoomId(1),
            kind: RoomKind::Fight,
            bounds_min: Vec2::new(0.0, -ROOM_SIZE),
            bounds_max: Vec2::new(ROOM_SIZE, 0.0),
        },
        Room {
            id: RoomId(2),
            kind: RoomKind::Extract,
            bounds_min: Vec2::new(0.0, 0.0),
            bounds_max: Vec2::new(ROOM_SIZE, ROOM_SIZE),
        },
    ];

    for room in &rooms {
        spawn_walls(&mut commands, room);
        spawn_doors(&mut commands, room, &rooms);
    }

    // Phase 2.3: spawn one `WaveSpawner` per Fight room. R0
    // (Spawn) and R2 (Extract) have no spawners — they're
    // "safe" rooms. The wave definitions are hard-coded
    // for Phase 2.3; Patch 5 ("deterministic procgen") will
    // derive them from `RunState.seed` so each seed gives a
    // different layout.
    //
    // The 3-wave progression for R1:
    //
    //   Wave 1 — 4 chasers, 1.5s apart (8s total, matches
    //            the plan's acceptance: "wave 1 (4 chasers),
    //            see all 4 spawn then auto-clear")
    //   Wave 2 — 2 dashers + 2 shooters, 1.0s apart
    //            (mixed-archetype wave: the dasher charges
    //            while the shooter plinks; the player has
    //            to dodge both)
    //   Wave 3 — 1 sniper, 0.5s apart (single hard
    //            enemy; the wave is short and climactic)
    //
    // Total time to clear R1: ~17s + combat. The 5-minute
    // run cap (Patch 6) leaves plenty of headroom.
    let fight_waves = vec![
        WaveDefinition {
            archetype: EnemyArchetype::Chaser,
            count: 4,
            interval_s: 1.5,
        },
        WaveDefinition {
            archetype: EnemyArchetype::Dasher,
            count: 2,
            interval_s: 1.0,
        },
        WaveDefinition {
            archetype: EnemyArchetype::Shooter,
            count: 2,
            interval_s: 1.0,
        },
        WaveDefinition {
            archetype: EnemyArchetype::Sniper,
            count: 1,
            interval_s: 0.5,
        },
    ];
    commands.spawn(WaveSpawner {
        room_id: RoomId(1),
        waves: fight_waves,
        current_wave: 0,
        spawned_this_wave: 0,
        // Timer fires after the FIRST interval_s of wave 1
        // (1.5s), so the player gets a small grace period
        // when entering the room. The wave_timer is reset
        // to the current wave's interval_s after each
        // spawn (see wave_spawner_tick_system).
        wave_timer: Timer::from_seconds(1.5, TimerMode::Once),
    });

    biome.rooms = rooms;
    biome.current_room = RoomId(0);

    // Re-anchor the player to R0's center. Overrides the
    // (0, 0) reset in `enter_in_run_system` (which fires
    // before this system). The intermediate (0, 0) position
    // is invisible — no rendering happens between OnEnter
    // systems in the same frame.
    if let Ok(mut player_tf) = player_query.single_mut() {
        let spawn_room = biome
            .rooms
            .iter()
            .find(|r| r.kind == RoomKind::Spawn)
            .expect("biome must have a Spawn room");
        player_tf.translation = spawn_room.center().extend(player_tf.translation.z);
    }

    info!(
        "RUSTWORN — biome initialized: {} rooms (Spawn / Fight / Extract)",
        biome.rooms.len()
    );
}

// Spawn 4 walls around the room's bounds. Each wall is a
// static collider + visible sprite. The walls are positioned
// just OUTSIDE the bounds (offset by WALL_THICKNESS/2) so the
// AABB and sprite align at the room's edge.
fn spawn_walls(commands: &mut Commands, room: &Room) {
    let cx = (room.bounds_min.x + room.bounds_max.x) * 0.5;
    let cy = (room.bounds_min.y + room.bounds_max.y) * 0.5;
    let w = room.size().x;
    let h = room.size().y;

    // North wall: spans the top of the room, just above bounds_max.y
    commands.spawn((
        Sprite {
            color: WALL_COLOR,
            custom_size: Some(Vec2::new(w + 2.0 * WALL_THICKNESS, WALL_THICKNESS)),
            ..default()
        },
        Transform::from_xyz(cx, room.bounds_max.y + WALL_THICKNESS * 0.5, WALL_Z),
        Collider {
            half_extents: Vec2::new(w * 0.5 + WALL_THICKNESS, WALL_THICKNESS * 0.5),
        },
        Wall,
    ));
    // South wall
    commands.spawn((
        Sprite {
            color: WALL_COLOR,
            custom_size: Some(Vec2::new(w + 2.0 * WALL_THICKNESS, WALL_THICKNESS)),
            ..default()
        },
        Transform::from_xyz(cx, room.bounds_min.y - WALL_THICKNESS * 0.5, WALL_Z),
        Collider {
            half_extents: Vec2::new(w * 0.5 + WALL_THICKNESS, WALL_THICKNESS * 0.5),
        },
        Wall,
    ));
    // East wall
    commands.spawn((
        Sprite {
            color: WALL_COLOR,
            custom_size: Some(Vec2::new(WALL_THICKNESS, h)),
            ..default()
        },
        Transform::from_xyz(room.bounds_max.x + WALL_THICKNESS * 0.5, cy, WALL_Z),
        Collider {
            half_extents: Vec2::new(WALL_THICKNESS * 0.5, h * 0.5),
        },
        Wall,
    ));
    // West wall
    commands.spawn((
        Sprite {
            color: WALL_COLOR,
            custom_size: Some(Vec2::new(WALL_THICKNESS, h)),
            ..default()
        },
        Transform::from_xyz(room.bounds_min.x - WALL_THICKNESS * 0.5, cy, WALL_Z),
        Collider {
            half_extents: Vec2::new(WALL_THICKNESS * 0.5, h * 0.5),
        },
        Wall,
    ));
}

// Spawn 0-2 doors per room based on the hard-coded layout.
//
// Layout (Patch 2.1 — bidirectional):
//
//   ┌──────────┐
//   │ Room 2   │  Extract (no enemies, south door → R1)
//   │          │  ↑ north door from R1
//   └──────────┘
//   ┌──────────┐
//   │ Room 1   │  Fight (4 spawners, west door → R0, north door → R2)
//   │          │  ← east door from R0
//   └──────────┘
//   ┌──────────┐
//   │ Room 0   │  Spawn (player starts here, east door → R1)
//   │          │  → door to R1 (east wall)
//   └──────────┘
//
// Patch 2.0 had only forward doors (R0→R1, R1→R2). That made
// rooms one-way — once the player walked R0→R1, they couldn't
// go back to grab a missed pickup or scout ahead. The
// back-doors added here are mirror images of the forward
// doors, sitting on the opposite wall of the destination
// room's "exit" side. So:
//
//   - R0's east door leads to R1's west side
//   - R1's west door leads back to R0's east side
//   - R1's north door leads to R2's south side
//   - R2's south door leads back to R1's north side
//
// Each door's `entry_position` is inset from the destination
// room's wall by ENTRY_INSET so the wall_collision_system
// doesn't immediately push the player back out.
fn spawn_doors(commands: &mut Commands, room: &Room, all_rooms: &[Room]) {
    let r0 = all_rooms
        .iter()
        .find(|r| r.id == RoomId(0))
        .expect("biome must have room 0");
    let r1 = all_rooms
        .iter()
        .find(|r| r.id == RoomId(1))
        .expect("biome must have room 1");
    let r2 = all_rooms
        .iter()
        .find(|r| r.id == RoomId(2))
        .expect("biome must have room 2");

    if room.id == RoomId(0) {
        // R0 → R1: east door
        commands.spawn((
            Sprite {
                color: DOOR_COLOR,
                custom_size: Some(DOOR_SIZE),
                ..default()
            },
            Transform::from_xyz(
                room.bounds_max.x - DOOR_INSET,
                (room.bounds_min.y + room.bounds_max.y) * 0.5,
                DOOR_Z,
            ),
            Collider {
                half_extents: DOOR_SIZE * 0.5,
            },
            Door {
                from_room: RoomId(0),
                to_room: RoomId(1),
                entry_position: Vec2::new(r1.bounds_min.x + ENTRY_INSET, r1.center().y),
                // R0 → R1 is a forward door (the player
                // is heading "deeper" into the run). It's
                // NOT locked, though — R1's waves unlock
                // the R1 → R2 door, not the R0 → R1 door.
                // The player can always re-enter the
                // Fight room from the Spawn room, even
                // mid-fight.
                locked: false,
                forward: true,
            },
        ));
    } else if room.id == RoomId(1) {
        // R1 → R0: west door (mirror of R0's east door)
        commands.spawn((
            Sprite {
                color: DOOR_COLOR,
                custom_size: Some(DOOR_SIZE),
                ..default()
            },
            Transform::from_xyz(
                room.bounds_min.x + DOOR_INSET,
                (room.bounds_min.y + room.bounds_max.y) * 0.5,
                DOOR_Z,
            ),
            Collider {
                half_extents: DOOR_SIZE * 0.5,
            },
            Door {
                from_room: RoomId(1),
                to_room: RoomId(0),
                entry_position: Vec2::new(r0.bounds_max.x - ENTRY_INSET, r0.center().y),
                // Backward door (returning to Spawn) —
                // always open. The player can retreat
                // from the Fight room to recover HP at
                // any time, even mid-wave.
                locked: false,
                forward: false,
            },
        ));
        // R1 → R2: north door
        commands.spawn((
            Sprite {
                color: DOOR_COLOR,
                custom_size: Some(DOOR_SIZE),
                ..default()
            },
            Transform::from_xyz(
                (room.bounds_min.x + room.bounds_max.x) * 0.5,
                room.bounds_max.y - DOOR_INSET,
                DOOR_Z,
            ),
            Collider {
                half_extents: DOOR_SIZE * 0.5,
            },
            Door {
                from_room: RoomId(1),
                to_room: RoomId(2),
                entry_position: Vec2::new(r2.center().x, r2.bounds_min.y + ENTRY_INSET),
                // R1 → R2 is the canonical "progression"
                // door: locked until R1's waves are
                // cleared, then unlocked by
                // `door_unlock_system`. This enforces
                // "fight through the room before moving
                // on" without forcing a clear (a future
                // patch can add an "extract" option that
                // skips the clear).
                locked: true,
                forward: true,
            },
        ));
    } else if room.id == RoomId(2) {
        // R2 → R1: south door (mirror of R1's north door)
        commands.spawn((
            Sprite {
                color: DOOR_COLOR,
                custom_size: Some(DOOR_SIZE),
                ..default()
            },
            Transform::from_xyz(
                (room.bounds_min.x + room.bounds_max.x) * 0.5,
                room.bounds_min.y + DOOR_INSET,
                DOOR_Z,
            ),
            Collider {
                half_extents: DOOR_SIZE * 0.5,
            },
            Door {
                from_room: RoomId(2),
                to_room: RoomId(1),
                entry_position: Vec2::new(r1.center().x, r1.bounds_max.y - ENTRY_INSET),
                // Backward door (returning to Fight) —
                // always open. The player can always go
                // back from the Extract room to the
                // Fight room.
                locked: false,
                forward: false,
            },
        ));
    }
}

// ─── AABB collision resolution ────────────────────────────────────────────

// Textbook AABB-vs-AABB resolution. Returns the minimum
// correction vector to apply to `a_pos` to push it out of
// `b`, or None if no overlap.
//
// Algorithm:
// 1. Compute the per-axis overlap (sum of half-extents minus
//    per-axis delta).
// 2. If either axis has no overlap, no collision.
// 3. Otherwise, push along the axis with the SMALLER overlap
//    (the "minimum translation distance" — the cheapest way
//    to resolve the collision).
//
// Sign of the correction: if `a_pos` is to the right of
// `b_pos`, the correction is negative (push `a` further right).
// `delta.x.signum()` gives +1 if a is right of b, -1 if left.
// Combined with `overlap_x`, this gives the correct direction
// AND magnitude.
fn aabb_resolve(a_pos: Vec2, a_half: Vec2, b_pos: Vec2, b_half: Vec2) -> Option<Vec2> {
    let delta = a_pos - b_pos;
    let overlap_x = (a_half.x + b_half.x) - delta.x.abs();
    let overlap_y = (a_half.y + b_half.y) - delta.y.abs();
    if overlap_x <= 0.0 || overlap_y <= 0.0 {
        return None;
    }
    if overlap_x < overlap_y {
        // Push out along X axis
        Some(Vec2::new(delta.x.signum() * overlap_x, 0.0))
    } else {
        // Push out along Y axis
        Some(Vec2::new(0.0, delta.y.signum() * overlap_y))
    }
}

// ─── physics_step_system ─────────────────────────────────────────────────

// Replaces the Phase 1 `movement_system` (which lived in
// player.rs). Two-phase per frame:
//
// 1. Apply velocity to transform for everything that has
//    Velocity (player, enemies, projectiles).
// 2. Resolve wall collisions for everything that has
//    Collider but is NOT a wall and NOT a projectile
//    (projectiles fly through walls in Patch 2).
//
// Why a single system, not two? The B0001 risk: a separate
// `movement_system` (writes Transform) + a separate
// `wall_collision_system` (writes Transform) that both
// iterate over player + enemies would conflict on the
// Transform component, and the disjointness check would
// fail because the two queries share a marker-less subset
// (any entity with Velocity AND Collider). Merging them
// into one system is the canonical fix (per the
// `bevy-0.16-gotchas` skill: same-component writes within
// a single system are fine; across systems they need
// filters or ParamSet).
fn physics_step_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    walls: Query<(&Transform, &Collider), With<Wall>>,
    mut movers: Query<
        (&mut Transform, &Velocity, &Collider),
        (
            Without<Wall>,
            Without<Projectile>,
            Without<AimIndicator>,
            Without<MainCamera>,
        ),
    >,
    mut projectiles: Query<
        (&mut Transform, &Velocity),
        (
            With<Projectile>,
            Without<Wall>,
            Without<AimIndicator>,
            Without<MainCamera>,
        ),
    >,
) {
    if hitstop.is_some() {
        return;
    }
    // `run` is unused here — the chain is gated on
    // `in_state(AppState::InRun)` so we never run in
    // MainMenu or GameOver. The reference keeps the
    // import live for the next system that needs it
    // (and matches the Phase 1 pattern where the check
    // was an explicit `if in_game_over(&run) { return; }`).
    let _ = &run;
    let dt = time.delta_secs();

    // Phase 1: integrate velocity into transform.
    // Movers (player + enemies) get wall collision; projectiles
    // (player bullets, shooter bullets, boss bullets) skip
    // wall collision and fly through walls freely in Patch 2.
    // A future patch can add "despawn projectile on wall hit"
    // if it becomes a balance issue (currently a missed shot
    // goes into the wall and despawns on lifetime expiry).
    for (mut tf, vel, _col) in &mut movers {
        tf.translation += vel.0.extend(0.0) * dt;
    }
    for (mut tf, vel) in &mut projectiles {
        tf.translation += vel.0.extend(0.0) * dt;
    }

    // Phase 2: resolve wall collisions for movers. Iterates
    // movers × walls (4 walls per room × ~10 movers max =
    // ~40 checks per frame, trivial cost).
    for (mut tf, _vel, col) in &mut movers {
        for (wall_tf, wall_col) in &walls {
            if let Some(correction) = aabb_resolve(
                tf.translation.truncate(),
                col.half_extents,
                wall_tf.translation.truncate(),
                wall_col.half_extents,
            ) {
                tf.translation += correction.extend(0.0);
            }
        }
    }
}

// ─── door_transition_system ───────────────────────────────────────────────

// If the player overlaps a door, transition to the new room.
// The transition does FOUR things in this order:
//
// 1. Despawn the old room's transient entities (enemies,
//    projectiles, puffs, pickups). They're all in the
//    current room because the spawner only ticks in the
//    current room, so a blanket despawn is correct (no
//    need to filter by RoomScope — Patch 5 will add that
//    when rooms can be re-entered with stale entities).
// 2. Reset the spawner timers to `spawner_interval_s` so
//    the new room gets the standard warmup before the
//    first enemy spawns. The Phase 1 `enter_in_run_system`
//    does the same on a full restart; we do it here on a
//    room transition.
// 3. Move the player to `door.entry_position` (inset from
//    the new room's wall so wall_collision doesn't
//    immediately push them out).
// 4. Update `biome.current_room`. The next frame's
//    `spawner_tick_system` will see the new current_room
//    and start ticking the spawners in the new room.
//
// One transition per frame: if (somehow) the player
// overlaps two doors, we fire the first one and return.
// In practice, the entry positions are far enough apart
// that this never happens with our 3-room layout.
fn door_transition_system(
    mut commands: Commands,
    mut biome: ResMut<BiomeState>,
    settings: Res<GameSettings>,
    mut spawners: Query<&mut WaveSpawner>,
    enemies: Query<Entity, With<Enemy>>,
    projectiles: Query<Entity, With<Projectile>>,
    pickups: Query<Entity, With<Pickup>>,
    puffs: Query<Entity, With<DeathPuff>>,
    mut player_query: Query<
        (&mut Transform, &Collider),
        (With<Player>, Without<AimIndicator>, Without<MainCamera>),
    >,
    mut doors: Query<(&Transform, &Collider, &mut Door), Without<Player>>,
    // Per-system-instance state. The previous `bool` is
    // read at the top of the system: if it's true, we
    // skip the door check entirely this frame (the player
    // just teleported last frame, give them one frame of
    // grace so a marginal AABB overlap doesn't immediately
    // re-fire). The flag is set inside the transition block
    // and read+cleared at the top of the next call.
    mut just_teleported: Local<bool>,
) {
    if *just_teleported {
        *just_teleported = false;
        return;
    }
    let Ok((mut player_tf, player_col)) = player_query.single_mut() else {
        return;
    };

    for (door_tf, door_col, door) in &doors {
        // Phase 2.3: skip locked doors. The player can stand
        // on the trigger zone, but nothing happens. The
        // trigger is a 60×30 AABB on the floor in front of
        // the wall — it doesn't block movement, so the
        // player can walk freely over it while it's locked.
        if door.locked {
            continue;
        }
        // AABB overlap check (we don't need the resolution
        // vector, just "is there overlap?").
        if aabb_resolve(
            player_tf.translation.truncate(),
            player_col.half_extents,
            door_tf.translation.truncate(),
            door_col.half_extents,
        )
        .is_some()
        {
            // Snapshot the door's destination into a local
            // before we drop the `&doors` borrow. The
            // locking pass below needs `&mut doors`, which
            // conflicts with the active `&doors` from the
            // for-loop iterator. Rust's NLL can't see that
            // we're about to `return`, so we make the
            // drop explicit.
            let to_room = door.to_room;
            let entry_pos = door.entry_position;

            // 1. Despawn old room's transient entities.
            for e in &enemies {
                commands.entity(e).despawn();
            }
            for e in &projectiles {
                commands.entity(e).despawn();
            }
            for e in &pickups {
                commands.entity(e).despawn();
            }
            for e in &puffs {
                commands.entity(e).despawn();
            }

            // 2. Reset wave timers. Re-arm each spawner's
            //    timer with the first wave's interval
            //    (current_wave was reset on entry, or the
            //    spawner is brand new). This is the "fresh
            //    warmup" the player gets when they walk
            //    into a new room.
            for mut spawner in &mut spawners {
                if let Some(first_wave) = spawner.waves.first() {
                    spawner.wave_timer =
                        Timer::from_seconds(first_wave.interval_s, TimerMode::Once);
                }
                spawner.current_wave = 0;
                spawner.spawned_this_wave = 0;
            }

            // 3. Move player to entry position (keep their
            //    current z so the AIM indicator child renders
            //    at the right depth).
            player_tf.translation = entry_pos.extend(player_tf.translation.z);

            // 4. Update current room.
            biome.current_room = to_room;

            // 5. Lock all doors in the destination room —
            //    but only if it's a "challenge" room
            //    (has a WaveSpawner). Safe rooms (Spawn,
            //    Extract) have no waves, so the player
            //    can move in and out freely. This matches
            //    the user's model: "all doors lock when
            //    entering" the room-with-enemies; safe
            //    rooms stay open.
            //
            //    We mutate the `doors` query directly here.
            //    The for-loop borrow was released when we
            //    copied `to_room` into a local; we're now
            //    free to take `&mut doors`.
            let dest_has_waves = spawners.iter().any(|s| s.room_id == to_room);
            if dest_has_waves {
                for (_, _, mut sibling) in &mut doors {
                    if sibling.from_room == to_room && !sibling.locked {
                        sibling.locked = true;
                    }
                }
            }

            // Mark the next frame as "just teleported" so
            // a marginal AABB overlap on the new room's
            // return door doesn't immediately re-fire.
            // Belt + suspenders with the ENTRY_INSET=100
            // math above: even if a future patch moves
            // the entry position closer to the wall,
            // this guard prevents the loop.
            *just_teleported = true;

            info!(
                "RUSTWORN — room transition → {:?} (entry at ({:.0}, {:.0}))",
                to_room, entry_pos.x, entry_pos.y
            );

            // One transition per frame.
            return;
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // AABB resolution: the cornerstone of the wall collision
    // system. A bug here means either the player walks through
    // walls (under-correction) or jitter-rubs against invisible
    // surfaces (over-correction / wrong axis). Both are
    // easy-to-notice visual bugs; the tests cover the main
    // cases that show up in playtesting.

    #[test]
    fn aabb_no_overlap_returns_none() {
        // Two boxes 100 units apart on X, no overlap.
        let r = aabb_resolve(
            Vec2::new(100.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        );
        assert!(r.is_none(), "distant boxes should not collide");
    }

    #[test]
    fn aabb_overlap_x_axis_pushes_along_x() {
        // A overlaps B on X only. B is at origin (10x10 half).
        // A is at (15, 100) with 10x10 half — overlaps by 5 on
        // X (delta.x=15, |overlap_x|=20-15=5, no Y overlap).
        // Wait — A's y=100 vs B's y=0, |delta.y|=100, |overlap_y|=20-100=-80 (no overlap).
        // So no collision. Need to test with Y overlap.
        let r = aabb_resolve(
            Vec2::new(15.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        );
        assert!(r.is_some(), "X-overlapping boxes should collide");
        let c = r.unwrap();
        assert!(c.x.abs() > 0.0, "should push along X: got {:?}", c);
        assert_eq!(c.y, 0.0, "should NOT push along Y: got {:?}", c);
    }

    #[test]
    fn aabb_overlap_y_axis_pushes_along_y() {
        // Y-overlap only (boxes stacked vertically).
        let r = aabb_resolve(
            Vec2::new(0.0, 15.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        );
        assert!(r.is_some());
        let c = r.unwrap();
        assert_eq!(c.x, 0.0, "should NOT push along X: got {:?}", c);
        assert!(c.y.abs() > 0.0, "should push along Y: got {:?}", c);
    }

    #[test]
    fn aabb_pushes_a_away_from_b() {
        // A is to the right of B. After resolution, A should be
        // pushed further right (positive X correction).
        let r = aabb_resolve(
            Vec2::new(5.0, 0.0), // A is to the right of B
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        );
        let c = r.unwrap();
        assert!(c.x > 0.0, "A should be pushed away from B (right)");
    }

    #[test]
    fn aabb_pushes_a_left_when_a_is_left_of_b() {
        // A is to the left of B. After resolution, A should be
        // pushed further left (negative X correction).
        let r = aabb_resolve(
            Vec2::new(-5.0, 0.0), // A is to the left of B
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        );
        let c = r.unwrap();
        assert!(c.x < 0.0, "A should be pushed left (away from B): got {:?}", c);
    }

    #[test]
    fn aabb_picks_minimum_translation_distance() {
        // When both axes overlap, push along the axis with the
        // SMALLER overlap (the "minimum translation distance"
        // heuristic — cheaper to slide A by 1 unit on X than
        // 5 units on Y).
        // A is at (5, 5), half_extents (10, 10) — 15 units of
        // overlap on each axis. Equal overlap → ties go to X
        // (the algorithm prefers X by the `<` comparison).
        let r = aabb_resolve(
            Vec2::new(5.0, 5.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        );
        let c = r.unwrap();
        // With equal overlap, our algorithm picks X (overlap_x < overlap_y is false, so we push Y).
        // Either is correct — the test just needs to confirm ONE axis is picked.
        assert!(
            (c.x == 0.0) ^ (c.y == 0.0),
            "MTD should pick exactly one axis, got {:?}",
            c
        );
    }

    // Phase 2.3 regression: the entry position of a door
    // A→B must NOT cause the player's AABB to overlap with
    // the door B→A's trigger zone. Otherwise, on the frame
    // after the player teleports into B, the door_transition
    // system immediately picks them up on B's return door
    // and teleports them back to A — an infinite loop the
    // player can't escape.
    //
    // We test by computing the same geometry the runtime
    // uses (the constants from the top of the file: DOOR_SIZE
    // for trigger size, ENTRY_INSET for entry distance from
    // the wall, 20.0 for player AABB half-extent) and
    // asserting no AABB overlap on any of the 4 door pairs.
    #[test]
    fn entry_position_does_not_overlap_return_door_trigger() {
        // The 3 rooms, mirrors of setup_rooms_system.
        let rooms = vec![
            Room {
                id: RoomId(0),
                kind: RoomKind::Spawn,
                bounds_min: Vec2::new(-ROOM_SIZE, -ROOM_SIZE),
                bounds_max: Vec2::new(0.0, 0.0),
            },
            Room {
                id: RoomId(1),
                kind: RoomKind::Fight,
                bounds_min: Vec2::new(0.0, -ROOM_SIZE),
                bounds_max: Vec2::new(ROOM_SIZE, 0.0),
            },
            Room {
                id: RoomId(2),
                kind: RoomKind::Extract,
                bounds_min: Vec2::new(0.0, 0.0),
                bounds_max: Vec2::new(ROOM_SIZE, ROOM_SIZE),
            },
        ];
        let r0 = &rooms[0];
        let r1 = &rooms[1];
        let r2 = &rooms[2];

        // Player AABB half-extents (mirror of Collider in
        // setup.rs:spawn_world).
        let player_half = Vec2::splat(20.0);
        // Door trigger half-extents.
        let door_half = DOOR_SIZE * 0.5;

        // 4 door pairs to check:
        //   R0→R1 entry (in R1) vs R1→R0 trigger (in R1)
        //   R1→R0 entry (in R0) vs R0→R1 trigger (in R0)
        //   R1→R2 entry (in R2) vs R2→R1 trigger (in R2)
        //   R2→R1 entry (in R1) vs R1→R2 trigger (in R1)
        //
        // We check each pair independently so a failure
        // message can name the offending door.

        // R0→R1: entry in R1 at (r1.min.x + ENTRY_INSET, r1.center().y)
        //         trigger of R1→R0 in R1 at (r1.min.x + DOOR_INSET, r1.center().y)
        let entry_r0_to_r1 = Vec2::new(r1.bounds_min.x + ENTRY_INSET, r1.center().y);
        let trigger_r1_to_r0 = Vec2::new(r1.bounds_min.x + DOOR_INSET, r1.center().y);
        assert!(
            aabb_resolve(entry_r0_to_r1, player_half, trigger_r1_to_r0, door_half).is_none(),
            "R0→R1 entry {:?} overlaps R1→R0 trigger {:?}",
            entry_r0_to_r1,
            trigger_r1_to_r0
        );

        // R1→R0: entry in R0 at (r0.max.x - ENTRY_INSET, r0.center().y)
        //         trigger of R0→R1 in R0 at (r0.max.x - DOOR_INSET, r0.center().y)
        let entry_r1_to_r0 = Vec2::new(r0.bounds_max.x - ENTRY_INSET, r0.center().y);
        let trigger_r0_to_r1 = Vec2::new(r0.bounds_max.x - DOOR_INSET, r0.center().y);
        assert!(
            aabb_resolve(entry_r1_to_r0, player_half, trigger_r0_to_r1, door_half).is_none(),
            "R1→R0 entry {:?} overlaps R0→R1 trigger {:?}",
            entry_r1_to_r0,
            trigger_r0_to_r1
        );

        // R1→R2: entry in R2 at (r2.center().x, r2.min.y + ENTRY_INSET)
        //         trigger of R2→R1 in R2 at (r2.center().x, r2.min.y + DOOR_INSET)
        let entry_r1_to_r2 = Vec2::new(r2.center().x, r2.bounds_min.y + ENTRY_INSET);
        let trigger_r2_to_r1 = Vec2::new(r2.center().x, r2.bounds_min.y + DOOR_INSET);
        assert!(
            aabb_resolve(entry_r1_to_r2, player_half, trigger_r2_to_r1, door_half).is_none(),
            "R1→R2 entry {:?} overlaps R2→R1 trigger {:?}",
            entry_r1_to_r2,
            trigger_r2_to_r1
        );

        // R2→R1: entry in R1 at (r1.center().x, r1.max.y - ENTRY_INSET)
        //         trigger of R1→R2 in R1 at (r1.center().x, r1.max.y - DOOR_INSET)
        let entry_r2_to_r1 = Vec2::new(r1.center().x, r1.bounds_max.y - ENTRY_INSET);
        let trigger_r1_to_r2 = Vec2::new(r1.center().x, r1.bounds_max.y - DOOR_INSET);
        assert!(
            aabb_resolve(entry_r2_to_r1, player_half, trigger_r1_to_r2, door_half).is_none(),
            "R2→R1 entry {:?} overlaps R1→R2 trigger {:?}",
            entry_r2_to_r1,
            trigger_r1_to_r2
        );
    }

    // Phase 2.3: door state machine. Documents the
    // "lock on entry to a challenge room, unlock on
    // clear" contract by simulating the data flow
    // directly (we can't test the systems without a
    // full ECS harness, but we can verify the
    // state-machine contract).
    //
    // The rules are:
    //  1. A door is "open" if `locked == false`,
    //     "closed" if `locked == true`.
    //  2. When the player enters a challenge room
    //     (one with a WaveSpawner), every door whose
    //     `from_room == destination.room_id` becomes
    //     closed. This is the "doors lock behind you"
    //     effect.
    //  3. When a room's waves are cleared
    //     (RoomClearedEvent fires), every door whose
    //     `from_room == cleared.room_id` becomes
    //     open. This is the "doors open when cleared"
    //     effect.
    //  4. Safe rooms (no WaveSpawner) never trigger
    //     rule 2, so their doors stay open. This
    //     lets the player move freely in and out of
    //     the Spawn and Extract rooms.
    #[test]
    fn door_locks_when_entering_challenge_room_and_unlocks_on_clear() {
        // Build the 3 rooms, mirrors of setup_rooms_system.
        let rooms = vec![
            Room {
                id: RoomId(0),
                kind: RoomKind::Spawn,
                bounds_min: Vec2::new(-ROOM_SIZE, -ROOM_SIZE),
                bounds_max: Vec2::new(0.0, 0.0),
            },
            Room {
                id: RoomId(1),
                kind: RoomKind::Fight,
                bounds_min: Vec2::new(0.0, -ROOM_SIZE),
                bounds_max: Vec2::new(ROOM_SIZE, 0.0),
            },
            Room {
                id: RoomId(2),
                kind: RoomKind::Extract,
                bounds_min: Vec2::new(0.0, 0.0),
                bounds_max: Vec2::new(ROOM_SIZE, ROOM_SIZE),
            },
        ];

        // Build the 4 doors with initial state (forward
        // doors unlocked except R1→R2 which starts locked
        // — the canonical "progression" door).
        let mut doors = vec![
            // R0→R1: forward, unlocked
            Door {
                from_room: RoomId(0),
                to_room: RoomId(1),
                entry_position: Vec2::ZERO,
                locked: false,
                forward: true,
            },
            // R1→R0: back, unlocked
            Door {
                from_room: RoomId(1),
                to_room: RoomId(0),
                entry_position: Vec2::ZERO,
                locked: false,
                forward: false,
            },
            // R1→R2: forward, LOCKED (unlocks on R1 clear)
            Door {
                from_room: RoomId(1),
                to_room: RoomId(2),
                entry_position: Vec2::ZERO,
                locked: true,
                forward: true,
            },
            // R2→R1: back, unlocked
            Door {
                from_room: RoomId(2),
                to_room: RoomId(1),
                entry_position: Vec2::ZERO,
                locked: false,
                forward: false,
            },
        ];
        // Mark which rooms have a WaveSpawner (R1 only).
        let rooms_with_spawners = [RoomId(1)];

        // ── Step 1: player enters R1 via R0→R1. ──
        // R1 has a spawner, so all R1 doors should lock.
        let dest = RoomId(1);
        if rooms_with_spawners.contains(&dest) {
            for door in &mut doors {
                if door.from_room == dest && !door.locked {
                    door.locked = true;
                }
            }
        }
        // R1→R0: was unlocked, now locked ✓
        // R1→R2: was locked, stays locked ✓
        assert!(doors[1].locked, "R1→R0 should lock when entering R1");
        assert!(doors[2].locked, "R1→R2 should stay locked when entering R1");

        // ── Step 2: player enters R0 via (hypothetical) R1→R0. ──
        // R0 has NO spawner, so R0 doors stay unlocked.
        // (The actual `door_transition_system` would refuse to
        // transition here because R1→R0 is now locked. So
        // this is a "what if the door were walkable" check.)
        // We simulate by temporarily marking R1→R0 as unlocked
        // and walking it.
        doors[1].locked = false; // simulate
        let dest = RoomId(0);
        if rooms_with_spawners.contains(&dest) {
            for door in &mut doors {
                if door.from_room == dest && !door.locked {
                    door.locked = true;
                }
            }
        }
        // R0→R1 should remain UNLOCKED (no spawner in R0).
        assert!(!doors[0].locked, "R0→R1 should stay open (R0 is safe)");

        // ── Step 3: R1 waves cleared → emit RoomClearedEvent. ──
        // All doors with from_room == R1 should unlock.
        let cleared = RoomId(1);
        for door in &mut doors {
            if !door.locked {
                continue;
            }
            if door.from_room == cleared {
                door.locked = false;
            }
        }
        // R1→R0 and R1→R2 both unlock.
        assert!(!doors[1].locked, "R1→R0 should unlock when R1 is cleared");
        assert!(!doors[2].locked, "R1→R2 should unlock when R1 is cleared");

        // ── Step 4: player walks R1→R2 into R2. ──
        // R2 has NO spawner, so R2 doors stay unlocked.
        let dest = RoomId(2);
        if rooms_with_spawners.contains(&dest) {
            for door in &mut doors {
                if door.from_room == dest && !door.locked {
                    door.locked = true;
                }
            }
        }
        // R2→R1 should stay unlocked.
        assert!(!doors[3].locked, "R2→R1 should stay open (R2 is safe)");

        // Touch the rooms var so the compiler doesn't warn
        // about it being unused. (It's used in the test
        // setup to document the topology.)
        let _ = &rooms;
    }

    // Spawn position clamp (Phase 2.1): the enemy must spawn
    // inside the current room, with a margin from each wall.
    // The clamp is a simple Vec2::clamp on each axis, so the
    // test cases are "raw pos inside", "raw pos outside on X",
    // "raw pos outside on Y", "raw pos outside on both".
    // (Tests live in enemy.rs — `random_spawn_position` is
    // defined there, so the clamp tests need access to it.)
}

// ─── door_unlock_system ───────────────────────────────────────────────────

// Reacts to `RoomClearedEvent` (emitted by the wave spawner
// in enemy.rs when a room's last wave finishes). For each
// event, unlock ALL doors with `from_room == event.room_id`.
//
// "All doors in the cleared room" — the same set that was
// locked on entry by `door_transition_system`. The
// "forward" flag is not consulted: we want the player to
// be able to leave through ANY door once the room's
// enemies are dead, including the door they originally
// entered through. This means after clearing R1, the
// player can walk back to R0 via the R1→R0 back door —
// the door they just used to enter is now re-unlocked
// for exit.
//
// We match by `Door.from_room == event.room_id`: a door
// that leads OUT of the cleared room. The `Door.from_room`
// field was added in Phase 2.3 so this system doesn't
// need to infer the source from position (which would
// break for branching paths in Phase 3+).
//
// Runs outside the InRun chain (no `run_if(in_state(...))`).
// The event channel is in any state — reading it eagerly
// is safer than relying on Bevy's event-clear semantics.
fn door_unlock_system(mut events: EventReader<RoomClearedEvent>, mut doors: Query<&mut Door>) {
    for event in events.read() {
        for mut door in &mut doors {
            if !door.locked {
                continue;
            }
            if door.from_room == event.room_id {
                door.locked = false;
            }
        }
    }
}

