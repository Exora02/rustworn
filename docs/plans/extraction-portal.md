# Extraction Portal Implementation Plan

> **For Hermes:** Use subagent-driven-development to implement this plan task-by-task.

**Goal:** Add an extraction portal that appears after each boss room clear — the player can either extract (end the run, earn currency) or push deeper (continue to the next floor with higher difficulty). This completes the core risk/reward run loop.

**Architecture:** New pad type "Extract" spawned alongside boss-room pads. Stepping on it triggers a run-end flow that tallies rewards, shows a summary, and transitions to MainMenu. A new `RunSummary` resource tracks completion data.

**Tech Stack:** Bevy 0.16 ECS.

---

### Design Decisions

1. **Pad type "Extract"** — reuse existing `TeleportPad` with label "Extract". Gold color contrasting with boss red.
2. **Appearance timing** — spawn alongside the boss-room exit pads (after boss death triggers `RoomClearedEvent`).
3. **Extraction action** — walking onto the pad sets `RunState::extracted = true`, fires an extraction event, and transitions to `AppState::GameOver` (but with a different overlay — "Extracted!" instead of "Game Over").
4. **Rewards** — on extraction, tally gold + XP + floor reached into a `RunSummary`. Future meta-progression will consume this.
5. **Run continuation** — player can choose to NOT step on the extract pad and instead take a "Fight" pad to the next biome. This is the existing behavior.
6. **GameOver distinction** — add `RunState::game_over_reason` enum: `Died` vs `Extracted`. The HUD overlay reads this to show different text.

**Files to touch:**
- `src/components/misc.rs` — add `GameOverReason` enum, `run_over_reason` to `RunState`
- `src/plugins/rooms/pads.rs` — spawn Extract pad alongside Boss pads
- `src/plugins/rooms/transitions.rs` — handle Extract pad: set reason, transition to GameOver
- `src/plugins/hud/overlay.rs` — show "EXTRACTED!" vs "GAME OVER"
- `src/resources/state.rs` — add `RunSummary` resource

---

### Task 1: Add GameOverReason + RunSummary

**Objective:** Data types for distinguishing extraction from death and tracking run results.

**Files:**
- Modify: `src/components/misc.rs` — add `GameOverReason`
- Modify: `src/resources/state.rs` — update RunState, add `RunSummary`

**Step 1: Add GameOverReason enum to misc.rs**

```rust
/// Why the run ended. Used by the HUD overlay to show different
/// end-of-run text ("GAME OVER" vs "EXTRACTED!").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GameOverReason {
    #[default]
    Died,
    Extracted,
}
```

Add to the re-export in `src/components/mod.rs`.

**Step 2: Add `game_over_reason` to `RunState`**

In `src/resources/state.rs`, add to `RunState`:
```rust
pub game_over_reason: GameOverReason,
```

Update `Default` impl:
```rust
game_over_reason: GameOverReason::Died,
```

**Step 3: Add `RunSummary` resource**

```rust
/// Set when the run ends (death or extraction). Consumed by the
/// HUD overlay and (future) meta-progression systems.
#[derive(Resource, Default)]
pub struct RunSummary {
    pub extracted: bool,
    pub floor_reached: u32,
    pub gold_earned: u32,
    pub xp_earned: u32,
    pub enemies_killed: u32,
    pub time_elapsed: f32,
}
```

Add to the re-export in `src/resources/mod.rs`.

---

### Task 2: Spawn Extract pad on boss room clear

**Objective:** After every boss room clear, spawn an extraction pad alongside the normal exit pads.

**Files:**
- Modify: `src/plugins/rooms/pads.rs`

**Step 1: Add Extract pad constant**

```rust
const PAD_COLOR_EXTRACT: Color = Color::srgb(0.20, 0.95, 0.40); // bright emerald green
```

**Step 2: Add Extract pad to `pad_spawn_on_clear_system`**

After the Boss pad spawn logic (line ~67-72), add:

```rust
        // Extract portal — always spawns if this is a boss room.
        // Player can step on it to end the run with rewards.
        // Skip the first boss cycle (floor 0 boss) to ensure the
        // player has enough time to learn the game before extraction
        // becomes meaningful.
        if next_is_boss && floor.floor_number > 0 {
            let extract_pos = center + Vec2::new(-PAD_SIZE * 1.2, 0.0);
            commands.spawn(pad_bundle(extract_pos, "Extract"));
        }
```

**Step 3: Add "Extract" to pad_bundle match**

```rust
"Extract" => PAD_COLOR_EXTRACT,
```

---

### Task 3: Handle extraction in transitions

**Objective:** When the player walks onto an Extract pad, end the run with extraction rewards.

**Files:**
- Modify: `src/plugins/rooms/transitions.rs`

**Step 1: Add Extract case to teleport_transition_system**

In the pad label match (around line 71-78), add:

```rust
"Extract" => {
    // End the run with extraction rewards.
    run_state.game_over_reason = GameOverReason::Extracted;
    next_state.set(AppState::GameOver);
    return; // Don't generate a new room.
}
```

Wait — `teleport_transition_system` currently doesn't have access to `next_state` or `run_state`. Let me check what it actually has...

The current signature (line 23-37) has `mut commands`, `mut floor`, `settings`, and various queries but no `NextState` or `RunState`. We need to add both.

Also — the current transition system generates a new room and moves the player into it. For extraction, we don't want to generate a room — we want to end the run.

To handle this cleanly: add `"Extract"` as a case that early-returns after setting game_over_reason and next_state. Don't despawn entities or generate rooms for Extract.

**Step 2: Add imports**

```rust
use crate::components::{..., GameOverReason};
use bevy::ecs::world::Mut;  // Actually just add NextState to bevy prelude
```

The `NextState` is in `bevy::prelude::*`.

---

### Task 4: Show "EXTRACTED!" in the HUD overlay

**Objective:** The game-over overlay shows different text based on `GameOverReason`.

**Files:**
- Modify: `src/plugins/hud/overlay.rs`

**Step 1: Read `RunState::game_over_reason`**

In the overlay's update system or spawn function, check `game_over_reason` and show:
- `Died` → "GAME OVER" (current behavior)
- `Extracted` → "EXTRACTED!" with a gold color

**Step 2: Set text color**

```rust
let (text, color) = match run_state.game_over_reason {
    GameOverReason::Died => ("GAME OVER", Color::srgb(0.85, 0.15, 0.15)), // red
    GameOverReason::Extracted => ("EXTRACTED!", Color::srgb(0.95, 0.85, 0.20)), // gold
};
```

---

### Task 5: Reset game_over_reason on new run

**Objective:** Ensure extraction state resets when starting a new run.

**Files:**
- Modify: `src/plugins/setup.rs` (or wherever `enter_in_run_system` lives)

The `enter_in_run_system` resets `RunState` to default. Since `Default` for `GameOverReason` is `Died`, this is already handled — but let's verify.

<hr>

### Task 6: Build check + commit

1. `cargo check`
2. Fix any compile errors
3. Commit: `feat: extraction portal — complete the run loop`