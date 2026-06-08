# Phase 2.3.2 — Design Shift: Gungeon-style Infinite Run

> **Status**: design proposal (awaiting user sign-off before implementation)
> **Date**: 2026-06-08
> **Author**: assistant (drafted from user feedback)
> **Replaces**: PLAN.md §1 ("5-minute sprint") and §6 ("3-5 biomes × 3 rooms")

This document captures the user's pivot from the original 5-minute-sprint design
to a longer-form Gungeon-style run. It identifies what stays, what changes, and
what new systems need to be designed.

---

## 1. What stays (carried over from the original plan)

- **Cascade Mode** (the primary twist). Tempo change, not power creep. 0.25× enemy
  speed for 30s, 4× XP / 4× loot / 4× damage taken. Stays as PLAN.md §2.
- **Modular procedural guns** (Barrel + Magazine + Stock + Optic + Modifier).
  PLAN.md §5 — unchanged.
- **Vampire Survivors-style auto-attack layer** once the player has 2+ weapons.
  PLAN.md §4 — unchanged.
- **Deterministic per seed** (xorshift64 RunRng, every random call goes through
  it). Already implemented in Phase 2 Patch 3 (`4e8a588`).
- **State machine doors** (lock on entry to challenge rooms, unlock on clear).
  Already implemented in Phase 2 Patches 3.1/3.2.

---

## 2. What changes (the shift)

### 2.1 Drop the 5-minute cap and the "extraction portal" mechanic

The original PLAN.md §1 called for a "5-minute sprint" with a hard 300s cap and
an `ExtractionPortal` at the end of each biome. **This is removed.** A run is
now an open-ended session that ends when the player dies.

Concretely, this means:
- `RunState.time_cap: f32` is removed (or set to `f32::INFINITY`).
- `ExtractionPortal` from Phase 2 Patch 6 is scrapped.
- `AppState::RunSummary` from Phase 2 Patch 6 is scrapped (the run ends at
  `AppState::GameOver` like a normal death).
- The "did I extract at the right time?" meta-decision (PLAN.md §7) is replaced
  by "can I survive one more room?" — a moment-to-moment decision rather than
  a per-biome extraction choice.

### 2.2 Bigger rooms, fewer of them per floor

Original PLAN.md §6: "Each biome = 3-room cluster (arena / treasure / gauntlet
/ boss)". The 3-room cluster was small and quick (the 90s micro-cycle in §3).

New design: **3-5 large rooms per floor**, each a 1-2 minute combat encounter
with ramping intensity. Total per-floor duration: 5-10 minutes of combat.

Room sizes (Phase 2 currently uses 800×800 units):
- **Small (regular combat)**: 800×800 — same as current rooms
- **Medium (mini-boss)**: 1200×1200 — bigger to fit a boss + arena hazards
- **Large (boss / gauntlet)**: 1600×1600 — biggest, with platforming elements

The wave system extends to support 5-8 waves per room (up from 4) with the
intensity curve:
- Wave 1: 4-6 chasers, slow spawn rate (1.5s)
- Wave 2: 6-8 mixed chasers + dashers, medium (1.0s)
- Wave 3: 4-6 shooters + 2 dashers, medium (1.0s)
- Wave 4: 2 snipers + 4 chasers, fast (0.5s)
- Wave 5: 1 mini-boss + 4 chasers, slow (2.0s)
- (if applicable) Wave 6: 1 boss, slow (3.0s)

### 2.3 Conquered rooms stay conquered

**Already implemented** in Phase 2 Patch 3.2 (`6c90b6a`):
- A room whose `WaveSpawner.current_wave >= waves.len()` is "conquered".
- On re-entry, the spawner is NOT reset (skip the wave-timer reset block in
  `door_transition_system`).
- The `wave_spawner_tick_system` already skips spawners past their last wave.

Phase 2 Patch 3.2 (this session) also added two regression tests:
- `conquered_room_spawner_not_reset_on_reentry` — locks in the fix
- `uncleared_room_spawner_is_reset_on_entry` — counter-test for fresh entry

### 2.4 Free teleport between any conquered room (FUTURE — not in Phase 2)

**Design intent**: once a room is conquered, the player can press a hotkey
(default: `T`) to open a "teleport menu" listing all conquered rooms, then
click one to instantly teleport there.

This is **not part of Phase 2** — it's a Phase 3+ feature that lands when we
have many rooms. But the data model needs to support it from Phase 2 forward:
- A `ConqueredRooms: HashSet<RoomId>` resource (or `BitSet<RoomId>` for
  performance) tracks which rooms the player has cleared.
- The existing `WaveSpawner.current_wave >= waves.len()` check is the
  source of truth for "is this room cleared?" — the `ConqueredRooms` set
  is a derived view that any system can read.

### 2.5 Infinite runs with exponential difficulty + build progression

The original plan has "3-5 biomes" (a fixed end point). The new design has
**infinite rooms** with two scaling axes.

> **Design principle (user feedback, 2026-06-08)**: don't ramp up too fast,
> runs should be able to last a while. The player should feel a sense of
> growing pressure across many rooms, not a sudden spike after room 3-4.
> Apply this to BOTH the difficulty curve AND the reward curve — the
> player should never feel like they out-gear the game in 5 rooms, but
> the difficulty should also not crush them before they've had time to
> build up.

**Difficulty scaling (per room cleared):**
- Enemy HP: ×1.08 per room (cumulative) — gentle ramp, ~2× HP by room 9
- Enemy damage: ×1.05 per room — even gentler
- Enemy count per wave: +1 per room (capped at a reasonable max)
- Spawn interval: ×0.95 per room (faster spawns)
- Mini-boss / boss HP scales the same as regular enemies

The "room 8-10 sweet spot" in the original draft was too aggressive — at
×1.15 / ×1.10 the player hits a wall at room 4-5, not room 8-10. The
gentler slopes (×1.08 / ×1.05) push the "where the build outpaces the
difficulty" crossover out to room 12-15, giving runs a real long tail.

**Build progression (per room cleared):**
- The player accumulates weapons (auto-attack slots fill up)
- Each weapon has rarities (PLAN.md §5) — rare drops are exponentially more
  powerful
- The build outpaces difficulty in the short term (Vampire Survivors-style
  power curve) but the curves cross around room 12-15 (where each room
  takes 2+ minutes to clear and one mistake = death)

**Reward curve (per room type):**
- Regular combat rooms: 5-15% drop chance on enemies, common-rarity-biased
- Mini-boss rooms: guaranteed rare+ drop (NOT epic+) — one tier above
  regular rooms, not two
- Regular boss rooms: guaranteed epic+ drop — only bosses give the
  top tier, and only one boss per floor
- Reward rooms: 2× drop chance (boost on top of the regular rate) plus
  1-2 guaranteed pickups on the ground

The "boss drops epic+" was the original §6 design. Keeping it — but
making regular combat rooms ALSO drop rewards (Isaac-style, see §3
table) means the player's build progresses every room, not just in
boss rooms. This avoids the "first 4 rooms are filler, then boss
gives everything" pattern.

This creates the "how deep can I go?" loop: each cleared room is a checkpoint
(you can teleport back to it for free), but the next room is harder, and
the run ends when the player can no longer keep up.

---

## 3. New room types (FUTURE — not in Phase 2)

Phase 2 stays with the current 3-room layout (R0 Spawn / R1 Fight / R2
Extract). The new room types land in Phase 3+, but the schema needs to
accommodate them:

| Room type | Enemy count | Reward | Notes |
|---|---|---|---|
| **Regular combat** | 4-6 enemies per wave, 4-5 waves | Common-rarity drops (5-15% per enemy) | The "Arena" type from §6 — now also drops rewards (Isaac-style) |
| **Mini-boss** | 1 mini-boss + 4 adds | Guaranteed **rare+** drop | One tier above regular; not epic (the boss's job) |
| **Regular boss** | 1 boss + adds | Guaranteed **epic+** drop | The only source of epic+ in a normal floor; one boss per floor |
| **Reward** | None (or weak adds) | 2-3 high-rarity pickups on the ground + 2× drop chance for the room | The "Treasure" type from §6 — now a drop-rate boost, not a separate type |
| **Gauntlet** | Timed (60s) escalating waves | Common drops | From §6 — time-attack variant |
| **Shop** | None | Buy/sell weapons | From §6 — meta-progression adjacent |
| **Safe / Hub** | None | Heal to full, weapons stash | R0 (Spawn) of each floor |

**Key change vs. original plan**: regular combat rooms can ALSO drop rewards
(Isaac-style). The original §6 had "Treasure" as a separate room type with
2× loot density; the new design has a flat drop chance on every enemy in
every room, with treasure rooms as a "2× drop chance" boost rather than a
separate type.

The "extract or push deeper" choice from PLAN.md §3 is gone (no extraction
portal). Instead, every cleared room is a checkpoint: the player can
return to it freely via teleport (when that feature lands), heal up, swap
weapons, then push forward.

---

## 4. Impact on the Phase 2 patch plan

The current `docs/phase_2_run_structure.md` has 7 patches. With the design
shift, the plan needs updating:

| Patch | Status | Change |
|---|---|---|
| P1: AppState refactor | DONE | unchanged |
| P2: Procedural arena (3 rooms) | DONE | unchanged |
| P3: Wave spawning | DONE | unchanged (but the "conquered" bug is fixed in P3.2) |
| **P3.2: Conquered rooms stay conquered** | DONE (this session) | NEW — `6c90b6a` |
| P4: Loot drops + weapon pickups | pending | extend: regular enemies also drop rewards (Isaac-style) |
| P5: Cascade beacon | pending | unchanged in scope, but `RunState.time_cap` is no longer the cascade cap |
| **P5b: Exponential difficulty scaling** | NEW | per-room-cleared HP/damage/count scaling |
| P6: ~~5-min cap + extraction portal~~ → **Infinite run support** | SCOPED DOWN | remove the portal + summary screen; runs end on death |
| P7: Determinism cleanup | pending | unchanged |

The new patches to add (Phase 3+):
- **P3+ A: Multi-floor procgen** — replace static 3-room layout with 3-5
  procedurally-generated large rooms
- **P3+ B: New room types** — mini-boss, regular boss, reward, shop
- **P3+ C: Conquered-rooms tracker** — `ConqueredRooms` resource +
  derivation from `WaveSpawner` state
- **P3+ D: Free teleport** — `T` hotkey + teleport menu UI
- **P3+ E: Difficulty scaling** — per-room scaling curves for HP/dmg/count

---

## 5. Acceptance criteria (revised for the shift)

Phase 2 (current) acceptance:
- A complete if short run — the player can start, fight, and finish (or
  die) without crashes or stuck states. **Still valid.**
- 2 biomes — replaced by "infinite rooms, exponential scaling".

Phase 3+ (post-shift) acceptance:
- Procedurally generated 3-5 room floors
- Each room is 1-2 minutes of combat
- Regular combat rooms drop rewards (Isaac-style)
- Mini-boss and boss rooms are larger and have unique enemy setups
- Cleared rooms stay cleared (already implemented)
- Player can teleport between any cleared room
- Difficulty scales per room cleared (HP, damage, count)
- Runs are open-ended; the player dies when they can't keep up

---

## 6. Open questions for the user

1. **Difficulty curve tuning**: settled at ×1.08 HP / ×1.05 damage per
   room (gentle, runs have a long tail — see §2.5). Can be tuned
   later via `GameSettings` once we have playtest data.
2. **Per-floor vs. per-room scaling**: should difficulty reset between
   floors (like Gungeon does) or accumulate across the whole run
   (like Vampire Survivors)?
3. **Reward-room "free heal"**: should cleared rooms be a place to heal
   to full HP, or should healing be limited to pickups and Cascade
   bonuses only?
4. **Boss room reward**: ✅ settled at "guaranteed epic+, mini-boss
   rare+, regular combat rooms can also drop common-rarity rewards".
   See §2.5 reward curve and §3 table. (User feedback: don't ramp up
   too fast either — runs should last a while.)
5. **Floor transition**: when the player clears a boss room, do they
   auto-advance to the next floor, or do they have a choice
   ("continue" / "teleport back to a previous boss")?

These are answered in the implementation phase, not now. The doc captures
the design intent; the code answers these questions with defaults that
can be tuned in `GameSettings`.

---

## 7. What lands in Phase 2 (current session)

**Just the bug fix**: the `WaveSpawner` reset-on-entry behavior that
re-spawned waves in cleared rooms.

Everything else (procgen, room types, teleport, difficulty scaling) is
Phase 3+ scope and will be broken into its own patches with their own
design docs.
