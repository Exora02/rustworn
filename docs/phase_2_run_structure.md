# Phase 2 — Run Structure

> **Goal**: turn rustworn from "an infinite open arena" into a complete
> 5-minute sprint with a title screen, a procedural arena (1 biome, 3
> rooms), wave-based enemy spawns, weapon drops, and an extraction portal.
> Per PLAN.md §10: "a complete if short run (2 biomes)".
>
> **Acceptance**: you boot the game, see the title screen, press ENTER
> to start a run, fight through 3 rooms with escalating waves, reach the
> extraction portal at 5min, and either extract (→ run summary) or push
> deeper (→ next biome).

This phase has 7 patches. Each one is independently testable and builds
on the last. Patches land in this order so the game is always playable
between commits.

---

## Patch 1: `AppState` refactor (DONE — `phase 2: appstate refactor`)

**What it does:** lifts `RunState.game_over: bool` into a real Bevy
`States` enum. Adds a title screen with "PRESS ENTER TO START" overlay.
Moves the Phase 1 key-gated `restart_system` to an `OnEnter(InRun)`
handler. The whole player / combat / enemy / pickup / fx chains are
gated on `in_state(AppState::InRun)`. The HUD runs in InRun + GameOver
(bars stay visible on the death screen, freeze at the player's death
position). The MainMenu overlay is visible only in MainMenu. The
game-over overlay is visible only in GameOver.

**New components:** `AppState { MainMenu, InRun, GameOver, Paused }`
enum, `HudElementKind::MainMenu` variant.

**State map:**

| State | Player | Spawners | HUD bars | GameOver overlay | MainMenu overlay | Cascade telegraph |
|---|---|---|---|---|---|---|
| MainMenu | hidden | don't tick | hidden | hidden | visible | hidden |
| InRun | visible | tick | visible | hidden | hidden | visible (if cascade active) |
| GameOver | visible (frozen) | don't tick | visible (frozen) | visible | hidden | hidden (frozen) |
| Paused | (unused) | (unused) | (unused) | (unused) | (unused) | (unused) |

**Per-state input:**
- MainMenu: ENTER → InRun, ESC → quit
- InRun: ESC → MainMenu (abandon run)
- GameOver: R → InRun (restart), ESC → MainMenu (skip restart)

**Files touched:**
- `src/components.rs` — added AppState + MainMenu variant
- `src/resources.rs` — removed `game_over: bool`
- `src/plugins/setup.rs` — state init, world spawn, OnEnter handler, visibility system, input systems
- `src/plugins/player.rs` — removed `in_game_over` helper, removed early-returns, gated chain on InRun, deleted `restart_system` (moved to setup)
- `src/plugins/combat.rs` — same
- `src/plugins/enemy.rs` — same
- `src/plugins/pickups.rs` — gated chain on InRun
- `src/plugins/fx.rs` — gated chain on InRun
- `src/plugins/hud.rs` — gated on InRun (cascade telegraph) and InRun|GameOver (hud_update), removed `run.game_over` visibility toggles
- `src/lib.rs` — removed `exit_on_esc`

**Validation:** `cargo check` / `build` / `clippy` clean (only pre-existing
stub dead-code warnings), 8s `play_loop.sh` exits OK (no panics).

---

## Patch 2: Procedural arena scaffold (1 biome, 3 rooms)

**Goal:** the player walks between 3 named rooms (spawn → fight →
extract) inside a single biome. Doors are entities; walking into a
door transitions to the next room.

**Schema additions:**
- `Room` component (id, kind, bounds)
- `Door { to_room: RoomId, position: Vec2 }` component
- `InRoom { room_id: RoomId }` marker on the player
- `Biome { current_room: RoomId, rooms: HashMap<RoomId, Room> }`
  resource

**Approach:** start with 3 **static** room layouts (hand-coded
positions for spawners, pickups, walls). No RNG yet — that's Patch
5 ("deterministic procgen"). The room data is a hard-coded const,
not a resource, to keep the first cut trivial. Doors are
`Collider` entities (AABB) at room edges; the player's contact
with a door triggers the transition.

**Camera follow per room:** the camera follow system lerps toward
the player, but on a room transition, snap to the new room's
center and let the player's normal movement re-anchor it.

**Wall colliders:** the rooms are bounded by static colliders
along the edges. The player can move freely inside, but can't
leave through walls (only through doors).

**Acceptance:** start a run, see the player in the spawn room,
walk into the door at the east edge, see the player in the fight
room (with 2 chaser spawners), walk to the south door, see the
extract room (empty for now, just the bounds).

---

## Patch 3: Wave spawning (deterministic per seed)

**Goal:** replace the 4 parallel `Spawner` entities (one per
archetype) with a single `WaveSpawner` that reads a
`WaveDefinition` per room. Per PLAN.md §6, all randomness is
seeded from `RunState.seed` so the run is reproducible.

**Schema additions:**
- `WaveDefinition { archetype: EnemyArchetype, count: u32, interval_s: f32 }`
- `WaveSpawner { waves: Vec<WaveDefinition>, current_wave: usize, wave_timer: Timer }`
- `RunRng { state: u64 }` — seeded RNG resource (replaces the
  ad-hoc xorshift in `boss_spawn_system` and `random_spawn_position`)

**Approach:**
1. `WaveSpawner` is spawned per room (in Patch 2's room setup)
2. Per wave, the spawner ticks `wave_timer`, spawns `count`
   enemies of `archetype` at `interval_s` between each
3. When the wave is done, advance `current_wave` and re-arm the
   timer for the next wave
4. When all waves are done, the room is "cleared" — no more
   enemies spawn, the door to the next room unlocks (a `Door.locked` bool)

**RunRng:** every `random_spawn_position` / `boss_spawn_angle` call
goes through `RunRng.next()` (xorshift64). Seed is `RunState.seed`,
which is set in `enter_in_run_system` from system time (Phase 3+
will let the player pick a seed for daily-challenge mode).

**Acceptance:** start a run, fight room 2's wave 1 (4 chasers),
see all 4 spawn then auto-clear, door unlocks, walk to room 3
(extract). Restart the run, see the same enemies in the same
positions (reproducible from seed).

---

## Patch 4: Loot drops + weapon pickups

**Goal:** enemies have a chance to drop a `WeaponPickup` on
death. Walking over a weapon pickup adds it to the player's
inventory (or swaps the active weapon — TBD on inventory model).

**Schema additions:**
- `WeaponPickup { archetype: WeaponArchetype }` component
- `WeaponInventory { weapons: [WeaponArchetype; 4] }` component
  on the player (4 slots, expand-to-6 is Phase 3+ meta-progression)
- `EnemyKilledEvent` already exists — extend it with
  `position: Vec2, killer: Option<Entity>` so the drop
  resolver can spawn the pickup at the right spot

**Rarity:** each `EnemyArchetype` has a `drop_chance: f32` on
`GameSettings`. Common enemies (Chaser) drop nothing or an
HP regen pickup (already exists). Rares (Sniper, Mini-boss) drop
a weapon pickup. Phase 4+ will add rarity tiers per PLAN.md §5.

**Acceptance:** kill a mini-boss, see a colored weapon pickup
spawn at its death position. Walk over it, the active weapon
swaps to the new archetype (with a HUD update on the fire-rate
bar color).

---

## Patch 5: Cascade 4× XP / 4× loot / Beacon enemy

**Goal:** fold in the Phase 1+ stretch items from `resources.rs`.
Adds:
- 4× XP multiplier on kills during Cascade
- 4× loot rarity roll (re-rolls the drop table with 4× weight
  on rare items) during Cascade
- A new `Beacon` enemy archetype — slower, more HP, no contact
  damage, but killing it during Cascade extends the window by
  5s (capped at +15s)

**Schema additions:**
- `Beacon` variant on `EnemyArchetype` + `AiBehavior`
- `BeaconState { ... }` per-entity state (similar to Boss)
- `cascade_beacon_alive: bool` on RunState — already exists, the
  flip-on-boss-death hook stays; new flip-on-beacon-death hook
  adds the 5s extension

**Beacon AI:** static (doesn't move). Spawns 1 per Cascade
window at a random distance from the player. Visible from the
moment the Cascade window opens (so the player knows there's
something to kill). Killing it extends the window.

**Acceptance:** during a Cascade window, a beacon appears. Kill
it, see the Cascade HUD bar fill back up by 5s. Pickups dropped
by enemies during Cascade are visibly more valuable (e.g. +
HP instead of tiny regen).

---

## Patch 6: 5-min cap + extraction portal

**Goal:** at 300s (the existing `RunState.time_cap`), an
`ExtractionPortal` spawns in the current room. Walking into it =
"RUN COMPLETE" with a summary screen. ESC from the summary = back
to MainMenu.

**Schema additions:**
- `ExtractionPortal` component
- `RunSummary { kills: u32, time: f32, cascade_kos: u32, weapons_used: [WeaponArchetype; 4] }` resource
- A second overlay (like the game-over screen) for the run summary

**Approach:**
1. `time_tick_system` checks `run.time_elapsed >= run.time_cap`
   → spawns an `ExtractionPortal` entity at the room center
2. `extraction_portal_collect_system` (gated on InRun) checks for
   player ↔ portal overlap, transitions to a new state
   `AppState::RunSummary` (or back to MainMenu with the
   summary cached)
3. The summary screen shows: kills, time, Cascade KOs, weapons used
4. ESC from the summary → MainMenu

**Acceptance:** survive for 5 minutes, see the portal appear,
walk into it, see the summary screen. Press ESC, return to the
title screen.

---

## Patch 7: Determinism cleanup

**Goal:** make every random call go through `RunRng` so the run
is reproducible per seed. The current code has a few
`rand::random()` and ad-hoc xorshift calls in:
- `boss_spawn_system` (angle calculation)
- `random_spawn_position` (enemy spawner)
- `cascade_beacon_alive` flipping (if a future patch uses RNG)

**Approach:** search for `rand::` and `SystemTime` / `UNIX_EPOCH`
in src/, route every randomness through `RunRng`. Add a
`scripts/replay.sh` that runs the same seed twice and diffs the
entity positions at t=60s (smoke test for non-determinism).

**Acceptance:** `replay.sh` shows identical entity positions at
the comparison frame for two runs with the same seed.

---

## Out of scope for Phase 2 (lands in Phase 3+)

- Procedural generation (real RNG, not just static room layouts)
- Meta-progression (Soul Imprint, Crown currency, save/load)
- Multiple biomes (right now we have 1 — biome 2+ lands in Phase 3)
- Audio (Phase 4)
- `bevy_hanabi` particles (still using sprite-based puffs + damage numbers)
- The 5th weapon (Launcher) — Phase 1 left it as a comment, can land anytime

---

## Risk map for Phase 2 (per `bevy-0.16-gotchas`)

- **B0001 cross-system:** Patch 2 adds 2-3 new systems per room
  (door transitions, room walls, run summary). The disjointness
  check on `&mut Transform` will fight us if any of them write
  to the player or to HUD entities. Mitigation: scope door /
  wall queries by their own marker (`Door`, `Wall`) and use
  `Without<Door>` / `Without<Wall>` on the player / HUD queries.
- **State machine ordering:** `OnEnter(InRun)` must fire AFTER
  the player entity exists (the handler touches it). The player
  is spawned in `Startup`, so this is automatic — but a future
  patch that spawns the player in `OnEnter(MainMenu)` needs to
  remember to spawn BEFORE transitioning to InRun.
- **NaN propagation:** the cascade telegraph position uses
  `player_tf.translation.truncate()`. If the player somehow ends
  up at NaN (e.g. a divide-by-zero in the damage system), the
  overlay disappears silently. Patch 6's extraction portal
  should add a `debug_assert!(pos.is_finite())` before the
  spawn to catch this.
- **Re-entrancy on enter_in_run_system:** Patch 3's wave
  spawner can fire on the same frame as `enter_in_run_system`
  if the spawner is spawned in `Startup` and its timer was
  ticking during MainMenu. Mitigation: reset the spawner
  timers in `enter_in_run_system` (already done for the 4
  parallel spawners; new wave spawner needs the same).
- **State transitions from within a system:** Patch 6's
  extraction-portal-collect-system needs to transition to
  `RunSummary` (a future state). Calling `next_state.set()`
  from within a system that also iterates entities is safe
  in Bevy 0.16 (the transition is queued, not immediate),
  but a future patch that reads `State<AppState>` immediately
  after will see the OLD state. Always read
  `state.get()` for "current" and `next_state.set()` for
  "queued".
