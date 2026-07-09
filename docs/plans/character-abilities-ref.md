# Character Ability Systems — Implementation Reference

## Key Mapping Convention
- **Space + E** = activate ability (global: dash on Space, ability on E)
- Actually, let's keep it simpler: **E key** triggers active ability on its own. Space is dash.

## Marauder — Grenade Throw
**File:** `src/plugins/combat/projectile.rs` or new `src/plugins/combat/abilities.rs`

Systems needed:
1. `marauder_grenade_system` — run on Input `KeyCode::KeyE` when `PlayerClass::Marauder`
   - 3s cooldown tracked on `RunState` or a new component
   - Spawn a grenade projectile entity: orange circle, travels in aim direction, explodes on contact with enemy or after 1.5s
   - Explosion: damage enemies within radius, despawn grenade

Components needed:
- `Grenade { timer: Timer, range: f32 }` component
- `MarauderCooldown` component (or just use RunState field)

## Reaver — Dash-strike
**File:** `src/plugins/player.rs`

Modify the existing dash system:
- When `PlayerClass::Reaver` and player dashes, check for enemies along the dash path
- For each enemy in a thin rectangle along the dash trajectory, apply damage
- Use the existing `Collider` and `Enemy` markers to detect hits
- Scale damage with `run.damage_stacks` multiplier

## Spectre — Teleport
**File:** `src/plugins/player.rs`

New system:
1. On `KeyCode::KeyE` when `PlayerClass::Spectre`:
   - Read `AimDirection` resource (the cursor direction)
   - Teleport player to `current_pos + aim_dir * 300.0`
   - Clamp to room bounds
   - Flash effect: spawn a brief colored sprite at origin and destination
   - 1.5s cooldown

## Engineer — Turret
**File:** new `src/plugins/combat/turret.rs`

Systems needed:
1. `engineer_turret_system` — On `KeyCode::KeyE` when `PlayerClass::Engineer`:
   - If no turret deployed, spawn one at player position
   - If turret deployed, despawn it (recall)
   - 4s cooldown between deploy/recall cycles

Components:
- `Turret { aim_timer: Timer }` — marker + AI component
- Turret entity: small blue square, auto-fires at nearest enemy

Turret AI (in Update, runs if Entity with Turret exists):
- Every 0.5s, find nearest enemy (same pattern as auto-attack)
- Spawn a small projectile toward it
- Projectile does 1 damage (base), scales with run.damage_stacks

## HUD integration
Add a new HudElementKind::AbilityCooldown that shows the active ability name and its cooldown bar below the HP bar. Only visible when not on MainMenu.