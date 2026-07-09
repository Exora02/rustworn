# Auto-Attack Layer Implementation Plan

> **For Hermes:** Use subagent-driven-development to implement this plan task-by-task.

**Goal:** Add the Vampire Survivors fusion auto-attack layer — when the player has 2+ weapons in their `WeaponInventory`, each non-active weapon automatically fires projectiles toward the nearest enemy on its own cooldown.

**Architecture:** A new `auto_attack_system` in `combat/` that reads `WeaponInventory` slots, tracks per-weapon cooldowns in a new component `AutoAttackCooldowns`, finds nearest enemies via spatial query, and spawns projectiles through the existing `weapon_stats_for` and projectile-spawn code paths.

**Tech Stack:** Bevy 0.16 ECS, existing projectile/weapon components.

---

### Design Decisions

1. **New component `AutoAttackCooldowns`** — a `HashMap<usize, Timer>` on the player entity mapping inventory slot index to its next-fire timer. This keeps cooldown state per-slot, independent of the active weapon's `fire_timer`.
2. **Nearest-enemy query** — `Query<&Transform, With<Enemy>>` each frame, compute closest by distance-squared. Simple O(n) per frame. With current enemy counts (<200), a per-frame scan is fine.
3. **Projectile spawn** — reuses the existing projectile-spawn code from `weapon_fire_system` (the same spawn logic, same component bundles). We'll extract a `spawn_projectile` helper to avoid duplication.
4. **Gating** — only fires when in `AppState::InRun`, not during hitstop, not during game over. Runs as part of the combat chain after `weapon_fire_system`.
5. **Auto-attack only when 2+ weapons owned** — check `WeaponInventory::slots.iter().filter(|s| s.is_some()).count() >= 2`.
6. **Auto-attack cooldown** — each non-active weapon fires at `fire_rate * 0.5` (half the active weapon's rate, so auto-attacks are supportive, not dominant).

**Files to touch:**
- `src/components/misc.rs` — add `AutoAttackCooldowns` component
- `src/plugins/combat/mod.rs` — add `auto_attack_system` to plugin builder, re-export
- `src/plugins/combat/auto_attack.rs` — the new system
- `src/plugins/combat/weapon.rs` — extract `spawn_projectile` helper

---

### Task 1: Extract `spawn_projectile` helper from `weapon_fire_system`

**Objective:** Extract the projectile-spawn logic into a `pub(crate)` function so both `weapon_fire_system` and the new `auto_attack_system` can use it.

**Files:**
- Modify: `src/plugins/combat/weapon.rs` (extract helper)
- Modify: `src/plugins/combat/mod.rs` (re-export if needed)

**Step 1: Read the current spawn code**

The spawn loop in `weapon_fire_system` (lines ~226-280) creates a projectile entity with Sprite, Transform, Velocity, Collider, Projectile components.

Extract into:
```rust
pub(crate) fn spawn_projectile(
    commands: &mut Commands,
    settings: &GameSettings,
    run: &RunState,
    player_pos: Vec2,
    dir: Vec2,
    damage: f32,
    projectile_speed: f32,
    pellet_count: u32,
    spread_rad: f32,
) {
```
The helper should:
- Accept all params needed for spawn (replacing what the loop currently reads from weapon/aim)
- Handle the spread loop internally
- Return nothing (spawns entities)

**Step 2: Refactor `weapon_fire_system` to call `spawn_projectile`**

Replace the inline spawn loop with a call to the helper. The burst state management stays in `weapon_fire_system`.

**Step 3: Update visibility**

`spawn_projectile` needs to be `pub(crate)` and accessible from `auto_attack.rs`. In `combat/mod.rs`, re-export it:
```rust
pub use weapon::spawn_projectile;
```

Or just import it from `super::weapon::spawn_projectile` in auto_attack.rs.

**Step 4: `cargo check`**

Run cargo check to verify the refactor didn't change behavior.

---

### Task 2: Add `AutoAttackCooldowns` component

**Objective:** Add the cooldown-tracking component for auto-attack per-slot timers.

**Files:**
- Modify: `src/components/misc.rs`

**Step 1: Add the component**

```rust
/// Tracks per-slot auto-attack cooldowns. Maps inventory slot index
/// (0..3) to its next-fire timer. Removed entirely when the player
/// has < 2 weapons (auto-attack disabled). Each timer is independent
/// so pistol in slot 1 and shotgun in slot 2 fire on separate cadences.
#[derive(Component, Clone)]
pub struct AutoAttackCooldowns {
    pub cooldowns: [Timer; 4],
}
```

**Step 2: Implement Default**

```rust
impl Default for AutoAttackCooldowns {
    fn default() -> Self {
        Self {
            // All timers start ready (finished = true).
            cooldowns: core::array::from_fn(|_| Timer::from_seconds(0.0, TimerMode::Once)),
        }
    }
}
```

---

### Task 3: Implement `auto_attack_system`

**Objective:** The core auto-attack logic: find nearest enemy, tick cooldowns, fire weapon projectiles from non-active slots.

**Files:**
- Create: `src/plugins/combat/auto_attack.rs`

**Step 1: Write the system**

```rust
use bevy::prelude::*;

use crate::components::{
    AutoAttackCooldowns, Enemy, Player, Transform, Weapon, WeaponArchetype, WeaponInventory,
};
use crate::resources::{GameSettings, Hitstop, RunState};
use crate::plugins::combat::weapon_stats_for;
use super::in_hitstop;

/// Auto-attack system (Vampire Survivors fusion).
///
/// When the player has 2+ weapons in their inventory, each non-active
/// weapon slot automatically fires projectiles toward the nearest enemy
/// on its own cooldown. The active weapon (Weapon component) is skipped
/// — it's already handled by `weapon_fire_system`.
///
/// Runs after `weapon_fire_system` in the combat chain. Gated on:
///   - AppState::InRun
///   - No global hitstop
///   - Player has >= 2 weapons in inventory
pub fn auto_attack_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    run: Res<RunState>,
    hitstop: Option<Res<Hitstop>>,
    mut commands: Commands,
    player_query: Query<(
        &Transform,
        &Weapon,
        &WeaponInventory,
        Option<&mut AutoAttackCooldowns>,
    ), With<Player>>,
    enemies: Query<&Transform, (With<Enemy>, Without<Player>)>,
) {
    if in_hitstop(&hitstop) {
        return;
    }

    let Ok((player_tf, active_weapon, inventory, cooldowns_opt)) = player_query.single() else {
        return;
    };

    let player_pos = player_tf.translation.truncate();

    // Count owned weapons
    let owned_count = inventory.slots.iter().filter(|s| s.is_some()).count();
    if owned_count < 2 {
        // Remove cooldowns component if it exists — no auto-attack needed
        if cooldowns_opt.is_some() {
            commands.entity(player_query.single().unwrap().0).remove::<AutoAttackCooldowns>();
        }
        return;
    }

    // Ensure cooldowns component exists
    let mut cooldowns = match cooldowns_opt {
        Some(c) => c,
        None => {
            commands.entity(player_query.single().unwrap().0).insert(AutoAttackCooldowns::default());
            // Re-fetch after insert
            let (_, _, _, new_cooldowns) = player_query.single().unwrap();
            new_cooldowns.unwrap()
        }
    };

    // Find nearest enemy
    let nearest = enemies
        .iter()
        .map(|e_tf| {
            let d = e_tf.translation.truncate().distance_squared(player_pos);
            (d, e_tf.translation.truncate())
        })
        .min_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let Some((_dist, target_pos)) = nearest else {
        return; // No enemies to target
    };

    let aim_dir = (target_pos - player_pos).normalize_or_zero();
    if aim_dir == Vec2::ZERO {
        return;
    }

    // Tick all cooldowns
    for cd in &mut cooldowns.cooldowns {
        cd.tick(time.delta());
    }

    // Fire each non-active weapon slot whose cooldown is ready
    for (slot_idx, slot) in inventory.slots.iter().enumerate() {
        let Some(archetype) = slot else {
            continue;
        };

        // Skip the active weapon slot (it fires via weapon_fire_system)
        if *archetype == active_weapon.archetype && slot_idx == active_slot_index(&inventory, active_weapon.archetype) {
            continue;
        }

        // We don't know the exact slot index of the active weapon,
        // so check by archetype match:
        if *archetype == active_weapon.archetype {
            continue;
        }

        if !cooldowns.cooldowns[slot_idx].finished() {
            continue;
        }

        // Look up weapon stats for this archetype
        let (damage, fire_rate, projectile_speed, pellet_count, spread_rad, _burst_rounds) =
            weapon_stats_for(*archetype);

        // Auto-attack fires at half rate of the active weapon
        let interval = 1.0 / (fire_rate * 0.5);
        cooldowns.cooldowns[slot_idx] = Timer::from_seconds(interval, TimerMode::Once);

        // Spawn projectile(s) aimed at nearest enemy
        // (Use the existing projectile spawn logic — for now inline it)
        let damage_mult = 1.0 + run.damage_stacks as f32 * settings.pickup_damage_per_stack;
        let final_damage = damage * damage_mult;

        spawn_projectile_entity(
            &mut commands,
            &settings,
            player_pos,
            aim_dir,
            final_damage,
            projectile_speed,
            pellet_count,
            spread_rad,
        );
    }
}

/// Helper to spawn a projectile entity (mirrors the logic in weapon_fire_system).
/// Extracted so both manual fire and auto-attack use the same spawn code.
fn spawn_projectile_entity(
    commands: &mut Commands,
    settings: &GameSettings,
    player_pos: Vec2,
    center_dir: Vec2,
    damage: f32,
    projectile_speed: f32,
    pellet_count: u32,
    spread_rad: f32,
) {
    use bevy::sprite::Anchor;
    use crate::components::{Collider, Projectile, Team, Velocity};
    use crate::plugins::combat::{PROJECTILE_COLOR, PROJECTILE_SIZE};
    use crate::plugins::combat::weapon::rotate_vec2;

    // (Same spawn logic as weapon_fire_system — copy the loop)
    for i in 0..pellet_count.max(1) {
        let angle_offset = if pellet_count == 1 {
            0.0
        } else {
            let t = i as f32 / (pellet_count - 1) as f32;
            (t - 0.5) * spread_rad
        };
        let dir = rotate_vec2(center_dir, angle_offset);
        let spawn_pos = player_pos + dir * settings.weapon_muzzle_offset;
        let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;

        commands.spawn((
            Sprite {
                color: PROJECTILE_COLOR,
                custom_size: Some(PROJECTILE_SIZE),
                ..default()
            },
            Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5)
                .with_rotation(Quat::from_rotation_z(angle)),
            Velocity(dir * projectile_speed),
            Collider {
                half_extents: Vec2::new(PROJECTILE_SIZE.x * 0.5, PROJECTILE_SIZE.y * 0.5),
            },
            Projectile {
                damage,
                lifetime: Timer::from_seconds(settings.projectile_lifetime_s, TimerMode::Once),
                team: Team::Player,
            },
        ));
    }
}
```

**Step 2: Fix — the `active_slot_index` problem**

We need to determine which inventory slot is active. Replace the "skip by archetype" logic with a more robust approach. Actually, the simplest approach: add an `active_slot` field to `AutoAttackCooldowns` or query it differently.

Better approach: track which slot is active by adding `active_slot: usize` to `WeaponInventory` and updating it in `weapon_switch_system`. This is clean and minimal.

**Step 3: `active_slot` field in `WeaponInventory`**

```rust
pub struct WeaponInventory {
    pub slots: [Option<WeaponArchetype>; 4],
    pub active_slot: usize,  // ← add this
}
```

Update `weapon_switch_system` to set `inventory.active_slot = target_slot_index`.

**Step 4: Update AutoAttackCooldowns**

The `active_slot` field on `WeaponInventory` means we can simplify auto_attack_system:

```rust
// Skip the active weapon slot
if slot_idx == inventory.active_slot {
    continue;
}
```

---

### Task 4: Wire into combat plugin

**Objective:** Register the new system in `CombatPlugin`.

**Files:**
- Modify: `src/plugins/combat/mod.rs`

**Step 1: Add module**

```rust
pub mod auto_attack;
```

**Step 2: Add system to the build chain**

Place it after `weapon_fire_system` and `beam_damage_system` in the chain order:

```rust
app.add_systems(
    Update,
    (
        weapon::weapon_cooldown_tick_system,
        weapon::weapon_switch_system,
        weapon::weapon_fire_system,
        auto_attack::auto_attack_system,  // ← new
        projectile::projectile_lifetime_system,
        projectile::projectile_damage_system,
        damage::beam_damage_system,
        damage::breakable_damage_system,
        // enemy projectiles added in their own chain
    )
        .chain()
        .run_if(in_state(AppState::InRun)),
);
```

**Step 3: `cargo check`**

Verify the build compiles.

---

### Task 5: Wire `active_slot` into `weapon_switch_system`

**Objective:** Update `weapon_switch_system` to set `WeaponInventory::active_slot` so auto_attack_system knows which slot is active.

**Files:**
- Modify: `src/plugins/combat/weapon.rs`
- Modify: `src/components/misc.rs` (add `active_slot` field)

**Step 1: Add active_slot to WeaponInventory**

```rust
pub struct WeaponInventory {
    pub slots: [Option<WeaponArchetype>; 4],
    pub active_slot: usize,
}
```

Update `Default` impl:
```rust
impl Default for WeaponInventory {
    fn default() -> Self {
        Self {
            slots: [Some(WeaponArchetype::Pistol), None, None, None],
            active_slot: 0,
        }
    }
}
```

**Step 2: Update weapon_switch_system**

Add `mut inventory_q: Query<&mut WeaponInventory, With<Player>>` to the system params. After switching, set `inventory.active_slot`:

```rust
pub fn weapon_switch_system(
    keys: Res<ButtonInput<KeyCode>>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut player_query: Query<&mut Weapon, With<Player>>,
    mut inventory_q: Query<&mut WeaponInventory, With<Player>>,
) {
    // ...existing logic...
    
    let Some(target) = target else { return; };
    
    // Map the target archetype to its slot index
    let Ok(inventory) = inventory_q.single_mut() else { return; };
    let target_slot = inventory.slots.iter().position(|s| *s == Some(target));
    if let Some(idx) = target_slot {
        inventory.active_slot = idx;
    }
    
    // ...rest of existing logic (weapon stats refresh)...
}
```

Wait — the current `weapon_switch_system` doesn't use `WeaponInventory` at all! It just sets the active `Weapon` component directly. That's fine: the inventory is the source of truth for *what you own*, the `Weapon` component is *what's active*. The switch system needs to read the inventory to find which slot matches the target archetype.

Actually, looking more carefully: the current switch system maps key presses directly to archetypes (Digit1 → Pistol, etc.), not to inventory slots. So pressing `2` always selects Shotgun, regardless of whether slot 1 has Shotgun. This means we need to map: "what archetype was just selected" → "which inventory slot has that archetype". If the archetype isn't in the inventory, it's a no-op.

Let me re-read the switch system more carefully...

The current system just picks an archetype by keypress and sets it on the `Weapon` component. It doesn't check the inventory at all. This works because the player's `Weapon` component starts as Pistol (from inventory slot 0), and when they pick up a new weapon, it's added to an empty slot — but the `Weapon` component is only updated when they press 1-4/Q, which directly sets a new archetype.

For auto-attack, we need `active_slot` to track which *inventory slot* is the active weapon, so we can skip firing from that slot. The simplest approach: after `weapon_switch_system` sets a new archetype on `Weapon`, also find which inventory slot contains that archetype and store the index.

```rust
// After setting weapon.archetype = target:
let Ok(mut inventory) = inventory_q.single_mut() else { return; };
inventory.active_slot = inventory.slots.iter().position(|s| *s == Some(target)).unwrap_or(0);
```

---

### Verification

1. `cargo check` — must pass clean
2. Logic check: with 1 weapon (Pistol in slot 0), no auto-attack fires
3. Logic check: pick up Shotgun → now 2 weapons → auto-attack fires Shotgun at nearest enemy on cooldown
4. Logic check: pressing 2 to switch to Shotgun → remaining weapon (Pistol) auto-attacks instead
5. Logic check: auto-attack respects hitstop
6. Logic check: auto-attack stops on game over