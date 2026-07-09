# Playable Characters Implementation Plan

> **For Hermes:** Use subagent-driven-development. Hard/blocking refactors go to glm 5.2.

**Goal:** 4 playable characters with unique abilities, selectable from a new character-select screen. The characters modify starting stats/abilities and add per-character systems.

**Tech Stack:** Bevy 0.16 ECS.

---

### Design

**Characters:**

| Character | Passive | Active (space) | Starting HP | Starting Speed |
|-----------|---------|----------------|-------------|----------------|
| **Marauder** | Knife AoE on kill (5% chance) | Grenade throw (3s cooldown) | 4 | 1.0 |
| **Reaver** | Lifesteal on kill (10% hp) | Dash-strike (2s, damages enemies in path) | 3 | 1.15 |
| **Spectre** | Faster HP regen (0.5× base) | Teleport to cursor (1.5s) | 3 | 1.0 |
| **Engineer** | Turret fires alongside player | Deploy/recall turret (4s) | 5 | 0.85 |

**Selection:** On the MainMenu before "PRESS ENTER TO START", player presses 1-4 to select character. A visual highlight shows the selection. The selection persists for the duration of the session.

**Character marker:** A `PlayerClass` component on the player entity set at spawn/reset. Systems check this to enable/disable character-specific behavior.

**Files to create/modify:**
- `src/components/player.rs` — add `PlayerClass` component
- `src/components/mod.rs` — re-export
- `src/resources/state.rs` — add `selected_class` to RunState
- `src/plugins/setup.rs` — character selection UI on MainMenu, apply on run start
- `src/plugins/player.rs` — character-specific ability systems
- `src/plugins/combat/mod.rs` — wire Marauder knife, Reaver lifesteal
- `src/plugins/hud/overlay.rs` — show selected class in shop menu

---

### Task 1: PlayerClass component + RunState.selected_class

**Add to `src/components/player.rs`:**

```rust
/// Which playable character the player chose this session.
/// Determines starting stats, passive, and active ability.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerClass {
    #[default]
    Marauder,
    Reaver,
    Spectre,
    Engineer,
}
```

**Add to `src/resources/state.rs`:**

Add `pub selected_class: PlayerClass` to `RunState`, default `PlayerClass::Marauder`.

Re-export in both `mod.rs` files.

---

### Task 2: Character select UI on MainMenu

**Modify `setup.rs` — spawn_world:**

Add text entities showing 4 character options below "PRESS ENTER TO START". A `HudElementKind::CharacterSelect` entity showing:

```
Select character (1-4):
1: Marauder  [selected]  — Knife AoE | Grenade
2: Reaver    [press 2]   — Lifesteal | Dash-strike
3: Spectre   [press 3]   — Regen     | Teleport
4: Engineer  [press 4]   — +1 HP     | Turret
```

**Modify `menu_input_system`:**

Handle 1-4 key presses to set `run_state.selected_class`. Update the text to show the current selection.

**Split input system:** Move character select into its own system `character_select_system` that runs before `menu_input_system` in the chain.

---

### Task 3: Apply character stats on run start

**In `enter_in_run_system`:**

After meta-upgrade application, match on `run.selected_class` to set:
- **Marauder**: default HP (4.0), default speed
- **Reaver**: `h.max = 3.0`, `settings.player_move_speed *= 1.15`
- **Spectre**: `h.max = 3.0`, `run.regen_per_sec += 0.5` (starts with more regen)
- **Engineer**: `h.max = 5.0`, `settings.player_move_speed *= 0.85`

---

### Task 4: Character ability systems

**Marauder — Grenade throw system:**
Detect Space+E (or dedicated key) → spawn grenade projectile that explodes on contact. 3s cooldown. Add to player plugin chain.

**Reaver — Dash-strike:**
Modify dash system: if `PlayerClass::Reaver`, the dash collision check damages enemies along the path.

**Spectre — Teleport:**
New system: Space+E → instantly move player to mouse cursor position. 1.5s cooldown.

**Engineer — Turret:**
New system: Space+E → spawn/detach a turret entity that auto-fires at nearest enemy. 4s cooldown max 1 deployed.

---

### Task 5: HUD integration

Show the selected character name/icon on the HUD (e.g. top-left corner).
Use a new `HudElementKind::ClassName` text entity that updates each frame.

---

### Task 6: Build check + commit