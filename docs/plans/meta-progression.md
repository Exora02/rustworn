# Meta-Progression Implementation Plan

> **For Hermes:** Use subagent-driven-development to implement this plan task-by-task.

**Goal:** Crown Currency earned on extraction/extraction + meta-upgrade tree + persistent save/load + a shop UI on the MainMenu screen.

**Architecture:** A new `CrownSave` resource for persistent state (loaded from disk on boot, saved on extraction), a `MetaConfig` resource mapping upgrade names to their costs/effects, and a dedicated `meta` plugin or addition to `setup.rs` for the upgrade shop.

**Tech Stack:** Bevy 0.16 ECS, serde+ron for persistence (already in Bevy's dependency tree).

---

### Design Overview

```
Run → Extraction → Earn Crown (based on floor + gold + kills)
                   → Save to disk
                   → MainMenu shows Crown balance + Upgrade grid
                   → Player buys upgrades with Crown
                   → Next run starts with upgrades applied
```

**Crown earn formula:** `floor_reached * 10 + gold_earned / 5 + kills * 2 + 5` (baseline)

**Upgrade types (Plan.md §7 inspired):**

| Upgrade | Cost | Max Level | Effect per level |
|---------|------|-----------|------------------|
| Max HP | 15 | 3 | +1 max HP |
| Move Speed | 20 | 3 | +10% move speed |
| Dash Cooldown | 15 | 3 | -0.2s dash cooldown |
| Starting Gold | 10 | 2 | Start with 10/20 gold |
| Extra Weapon Slot | 50 | 1 | 5th inventory slot |
| Cascade Duration | 25 | 2 | +3s per level |
| Damage | 30 | 3 | +5% damage per level |
| Regen | 25 | 2 | +0.5 HP/sec per level |

**Persistence:** `crown_save.ron` in the project directory. Loaded at startup, auto-saved on extraction and on every upgrade purchase.

**Files to create/modify:**
- `src/resources/meta.rs` — `CrownSave` resource, `MetaUpgrade` enum, `MetaConfig` resource
- `src/resources/mod.rs` — wire up new module
- `src/plugins/meta.rs` — `MetaPlugin`: save/load, upgrade shop UI, Crown earn on extraction
- `src/plugins/mod.rs` — register MetaPlugin
- `src/lib.rs` — add MetaPlugin
- `src/plugins/setup.rs` — apply upgrades on InRun entry

---

### Task 1: Create meta resources (CrownSave, MetaUpgrade, MetaConfig)

**Files:**
- Create: `src/resources/meta.rs`
- Modify: `src/resources/mod.rs`

`src/resources/meta.rs`:
```rust
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Persistent Crown data: saved between runs, loaded on boot.
#[derive(Resource, Serialize, Deserialize, Clone, Default)]
pub struct CrownSave {
    /// Total Crown currency the player has saved up.
    pub crown: u32,
    /// Purchased upgrades. Maps upgrade kind to current level (0 = not owned).
    pub upgrades: [u32; 8],
}

/// Which upgrade slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetaUpgradeKind {
    MaxHp = 0,
    MoveSpeed = 1,
    DashCooldown = 2,
    StartingGold = 3,
    ExtraSlot = 4,
    CascadeDuration = 5,
    Damage = 6,
    Regen = 7,
}

/// In-memory lookup of upgrade costs and effects.
#[derive(Resource)]
pub struct MetaConfig {
    pub upgrades: [MetaUpgradeDef; 8],
}

pub struct MetaUpgradeDef {
    pub kind: MetaUpgradeKind,
    pub name: &'static str,
    pub description: &'static str,
    pub base_cost: u32,
    pub cost_per_level: u32,
    pub max_level: u32,
}

impl Default for MetaConfig {
    fn default() -> Self {
        Self {
            upgrades: [
                MetaUpgradeDef { kind: MetaUpgradeKind::MaxHp, name: "Max HP", description: "+1 max HP", base_cost: 15, cost_per_level: 0, max_level: 3 },
                MetaUpgradeDef { kind: MetaUpgradeKind::MoveSpeed, name: "Move Speed", description: "+10% move speed", base_cost: 20, cost_per_level: 0, max_level: 3 },
                MetaUpgradeDef { kind: MetaUpgradeKind::DashCooldown, name: "Dash Cooldown", description: "-0.2s dash cooldown", base_cost: 15, cost_per_level: 0, max_level: 3 },
                MetaUpgradeDef { kind: MetaUpgradeKind::StartingGold, name: "Starting Gold", description: "Start with 10 gold", base_cost: 10, cost_per_level: 0, max_level: 2 },
                MetaUpgradeDef { kind: MetaUpgradeKind::ExtraSlot, name: "Extra Slot", description: "5th weapon slot", base_cost: 50, cost_per_level: 0, max_level: 1 },
                MetaUpgradeDef { kind: MetaUpgradeKind::CascadeDuration, name: "Cascade Duration", description: "+3s per level", base_cost: 25, cost_per_level: 0, max_level: 2 },
                MetaUpgradeDef { kind: MetaUpgradeKind::Damage, name: "Damage", description: "+5% damage", base_cost: 30, cost_per_level: 0, max_level: 3 },
                MetaUpgradeDef { kind: MetaUpgradeKind::Regen, name: "Regen", description: "+0.5 HP/sec", base_cost: 25, cost_per_level: 0, max_level: 2 },
            ],
        }
    }
}
```

In `src/resources/mod.rs`, add:
```rust
pub mod meta;
pub use meta::{CrownSave, MetaConfig, MetaUpgradeKind, MetaUpgradeDef};
```

Also add `serde` to Cargo.toml dependencies:
```toml
serde = { version = "1", features = ["derive"] }
ron = "0.8"
```

---

### Task 2: Save/load CrownSave from disk

**Files:**
- Create: `src/plugins/meta.rs`
- Modify: `src/plugins/mod.rs` — add `pub mod meta;`

`src/plugins/meta.rs` — Core plugin:

```rust
use bevy::prelude::*;
use log::info;
use std::fs;
use std::path::PathBuf;

use crate::resources::{CrownSave, GameSettings, MetaConfig, RunState, RunSummary};

const SAVE_PATH: &str = "crown_save.ron";

pub struct MetaPlugin;

impl Plugin for MetaPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<MetaConfig>()
            .add_systems(Startup, load_save)
            .add_systems(Update, on_extraction_earn_crown.run_if(in_state(crate::components::AppState::GameOver)));
    }
}

fn save_path() -> PathBuf {
    PathBuf::from(SAVE_PATH)
}

fn load_save(mut commands: Commands) {
    let path = save_path();
    let save = if path.exists() {
        match fs::read_to_string(&path) {
            Ok(data) => ron::from_str(&data).unwrap_or_default(),
            Err(e) => {
                info!("Failed to read Crown save: {}. Starting fresh.", e);
                CrownSave::default()
            }
        }
    } else {
        info!("No Crown save found — starting fresh.");
        CrownSave::default()
    };
    commands.insert_resource(save);
}

fn write_save(save: &CrownSave) {
    let path = save_path();
    if let Ok(data) = ron::to_string(save) {
        let _ = fs::write(&path, data);
    }
}

/// Earn Crown on extraction (not on death).
fn on_extraction_earn_crown(
    run: Res<RunState>,
    run_summary: Option<Res<RunSummary>>,
    mut save: ResMut<CrownSave>,
) {
    if run.game_over_reason != crate::components::GameOverReason::Extracted {
        return;
    }
    // Only earn once — check if we already processed this extraction
    // by checking if run_summary has been consumed.
    // (Simple: just earn based on current run state.)
    let floor = run.biome_index.max(1);
    let gold = run.gold;
    let kills = run.enemies_killed;
    let earned = floor * 10 + gold / 5 + kills * 2 + 5;
    save.crown += earned;
    write_save(&save);
    info!("Extraction: earned {} Crown (total: {})", earned, save.crown);
}
```

---

### Task 3: Apply upgrades on InRun start

Modify `enter_in_run_system` in `setup.rs` to apply purchased upgrades to the player on run start.

Add `save: Res<CrownSave>` and `meta: Res<MetaConfig>` to the system parameters.

Apply:
- **Max HP**: `h.max += save.upgrades[0] as f32` (each level = +1 max HP)
- **Move Speed**: store multiplier in `GameSettings.player_move_speed` or apply to `RunState`
- **Dash Cooldown**: `settings.player_dash_cooldown_s -= save.upgrades[2] as f32 * 0.2`
- **Starting Gold**: `run.gold = save.upgrades[3] * 10`
- **Extra Slot**: expand WeaponInventory to 5 slots
- **Cascade Duration**: `settings.cascade_duration_s += save.upgrades[5] as f32 * 3.0`
- **Damage**: store multiplier for `pickup_damage_per_stack` in RunState
- **Regen**: `run.regen_per_sec += save.upgrades[7] as f32 * 0.5`

---

### Task 4: Upgrade shop UI on MainMenu

In `setup.rs` / `spawn_world`, add a panel on MainMenu showing:
- Crown balance at top
- 8 upgrade rows: name | level/max | cost | Buy key prompt
- Player presses 1-8 to buy the corresponding upgrade
- ESC exits the shop panel and shows "PRESS ENTER TO START"

This is the most visual piece. Approach: spawn a `HudElementKind::MetaShop` overlay (hidden in InRun/GameOver, visible in MainMenu). It's a set of text entities with the upgrade list. The `menu_input_system` handles 1-8 key presses for purchases.

Entity structure:
- Crown balance text (one entity)
- 8 upgrade-row texts (eight entities, one per upgrade)
- All under `HudElementKind::MetaShop`
- Positioned relative to the player (same as other HUD elements)

---

### Task 5: Wire MetaPlugin into app

In `src/lib.rs`:
```rust
use crate::plugins::{
    CombatPlugin, EnemyPlugin, FxPlugin, HudPlugin, MetaPlugin, PickupsPlugin, PlayerPlugin,
    RoomsPlugin, SetupPlugin,
};
```
Add `MetaPlugin` to the plugin list.

In `src/plugins/mod.rs`:
```rust
pub mod meta;
pub use meta::MetaPlugin;
```

---

### Task 6: Build check + commit

1. `cargo check`
2. Fix errors
3. Commit: `feat: meta-progression — Crown currency, upgrades, save/load, shop UI`