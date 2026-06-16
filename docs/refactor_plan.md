# Proposed Module Split for Rustworn

Current state: 14 source files, 7 plugins (setup.rs, player.rs, combat.rs, enemy.rs, rooms.rs, hud.rs, pickups.rs, fx.rs, + the monolithic components.rs)

## Target: ~25 small modules

### 1. `components.rs` → split by domain
Current 1073 lines = 118 types. Split into:
- **components/enemy.rs** — AiBehavior, EnemyArchetype, Enemy state types (DasherState, ShooterState, SniperState, BomberState, OrbiterState, BossState)
- **components/player.rs** — Player, Weapon, WeaponArchetype, Projectile, Team, Health, Collider
- **components/world.rs** — Room, RoomKind, RoomId, Door, RoomBounds, Biome, FloorState
- **components/ui.rs** — HudElement, HudElementKind, all HUD bar types
- **components/misc.rs** — Shrine, Pickup, PickupKind, RunState fields, etc.

### 2. `enemy.rs` → split by archetype
Current 2011 lines = spawn + AI + contact damage + boss. Split into:
- **enemy/spawn.rs** — spawn_chaser, spawn_dasher, spawn_shooter, spawn_sniper, spawn_bomber, spawn_orbiter + spawn helper fns
- **enemy/ai.rs** — enemy_ai_system (the big match on AiBehavior)
- **enemy/contact.rs** — enemy_contact_damage_system
- **enemy/boss.rs** — boss-specific: spawn_boss_projectile, boss_ai, boss_phase2 burst
- **enemy/death_aoe.rs** — bomber_death_aoe_system (new)
- **enemy/waves.rs** — director_pick, WaveSpawner, director budget logic

### 3. `combat.rs` → split by weapon/combat concern
- **combat/weapon.rs** — weapon_fire_system, weapon_cooldown_tick_system, weapon_switch_system
- **combat/projectile.rs** — projectile_lifetime_system, projectile_damage_system
- **combat/events.rs** — event wiring: DamageEvent, EnemyKilledEvent, RoomClearedEvent

### 4. `rooms.rs` → split by generation phase
- **rooms/generate.rs** — generate_room (the big match on RoomKind)
- **rooms/pads.rs** — pad spawning, teleport transitions, pad_spawn_on_clear
- **rooms/shop.rs** — shop-specific: spawn_shop_items, shop_collect_system
- **rooms/transitions.rs** — teleport_transition_system, door_unlock_system

### 5. `player.rs` → split into movement/input/death
- Already relatively clean at 686 lines — but could split:
  - **player/movement.rs** — player_input_system, movement_system, velocity_apply_system
  - **player/dash.rs** — dash_input_system, dash_state_system
  - **player/state.rs** — gates, restart_system, player_death_check_system

### 6. `resources.rs` → split into resources/game_settings
- **resources/state.rs** — RunState, RoomState, FloorState
- **resources/tuning.rs** — GameSettings (the big Default impl)
- **resources/{rng,etc}.rs** — RunRng, Hitstop, ScreenShake, etc.

## Effort: ~3-4h of focused split work
- Each split is a mechanical `cut {region} → {new file}` with `pub use` re-exports
- No logic changes — just file structure
- submodule re-export pattern: `pub mod ai; pub mod spawn;` in each new directory
- `lib.rs` just gets `pub mod plugins;` → `mod plugins` auto-discovers

## Benefit
- Each file < 300 lines → easy to `read_file` without pagination
- `git diff` per PR is 300-500 lines instead of 2000
- Parallel AI subagents can work on different files simultaneously
- Tests in `enemy/ai.rs` vs `enemy/waves.rs` vs `enemy/spawn.rs` don't conflict