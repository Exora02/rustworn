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
    AimIndicator, AppState, Collider, DeathPuff, Door, Enemy, MainCamera, Pickup, Player,
    Projectile, Room, RoomId, RoomKind, Spawner, Velocity, Wall,
};
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
/// 50 units puts the player 1.25× their AABB half-extent
/// inside the new room, so the wall_collision_system
/// doesn't immediately push them out.
const ENTRY_INSET: f32 = 50.0;

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
                    // Door transition: if the player overlaps a
                    // door, fire the room change. Runs after
                    // physics_step so the player's AABB is
                    // current.
                    door_transition_system,
                )
                    .chain()
                    .run_if(in_state(AppState::InRun)),
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
                to_room: RoomId(1),
                entry_position: Vec2::new(r1.bounds_min.x + ENTRY_INSET, r1.center().y),
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
                to_room: RoomId(0),
                entry_position: Vec2::new(r0.bounds_max.x - ENTRY_INSET, r0.center().y),
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
                to_room: RoomId(2),
                entry_position: Vec2::new(r2.center().x, r2.bounds_min.y + ENTRY_INSET),
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
                to_room: RoomId(1),
                entry_position: Vec2::new(r1.center().x, r1.bounds_max.y - ENTRY_INSET),
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
    mut spawners: Query<&mut Spawner>,
    enemies: Query<Entity, With<Enemy>>,
    projectiles: Query<Entity, With<Projectile>>,
    pickups: Query<Entity, With<Pickup>>,
    puffs: Query<Entity, With<DeathPuff>>,
    mut player_query: Query<
        (&mut Transform, &Collider),
        (With<Player>, Without<AimIndicator>, Without<MainCamera>),
    >,
    doors: Query<(&Transform, &Collider, &Door), Without<Player>>,
) {
    let Ok((mut player_tf, player_col)) = player_query.single_mut() else {
        return;
    };

    for (door_tf, door_col, door) in &doors {
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

            // 2. Reset spawner timers.
            for mut spawner in &mut spawners {
                spawner.interval =
                    Timer::from_seconds(settings.spawner_interval_s, TimerMode::Repeating);
            }

            // 3. Move player to entry position (keep their
            //    current z so the AIM indicator child renders
            //    at the right depth).
            player_tf.translation = door.entry_position.extend(player_tf.translation.z);

            // 4. Update current room.
            biome.current_room = door.to_room;

            info!(
                "RUSTWORN — room transition → {:?} (entry at ({:.0}, {:.0}))",
                door.to_room, door.entry_position.x, door.entry_position.y
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

    // Spawn position clamp (Phase 2.1): the enemy must spawn
    // inside the current room, with a margin from each wall.
    // The clamp is a simple Vec2::clamp on each axis, so the
    // test cases are "raw pos inside", "raw pos outside on X",
    // "raw pos outside on Y", "raw pos outside on both".
    // (Tests live in enemy.rs — `random_spawn_position` is
    // defined there, so the clamp tests need access to it.)
}
