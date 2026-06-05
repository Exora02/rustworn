# RUSTWORN — Game Design Plan v0.1

> A 2D top-down roguelike in Bevy, fusing Nuclear Throne, Enter the Gungeon, Vampire Survivors, and Megabonk — with a tempo-based twist that flips the game every 60-90 seconds.

---

## 1. Elevator Pitch

RUSTWORN is a 2D top-down roguelike built in Bevy where every run is a **5-minute sprint** through a procedurally collapsing world. You fuse:

- **Nuclear Throne's** twitch twin-stick aim and mutation pickups
- **Enter the Gungeon's** curated gun feel and dodge-rolling
- **Vampire Survivors'** exponential auto-attack scaling
- **Megabonk's** screen-bending mobility (dash, slide, wall-kick)

The **twist**: every 60-90 seconds the game enters **Cascade Mode** — a 30-second slow-motion window where time runs at 0.25× for everyone except you. Kills are worth 4×, but so is damage taken. It's a roguelike that *breathes* — calm exploration, then sudden intensity.

---

## 2. The Twist: Cascade Mode (and 2 backups)

### Primary: Cascade Mode

**Mechanic**
- Every 60-90s, a 30s Cascade window opens.
- All enemies, enemy projectiles, and enemy particles scale to 0.25× speed.
- Player and player projectiles run at full speed.
- Kills during Cascade: 4× XP, 4× loot rarity roll.
- Damage taken during Cascade: 4×.
- **Chain bonus**: killing a "Beacon" enemy during Cascade extends the window by 5s (capped at +15s total).

**Visual & audio telegraph**
- Screen tints indigo, chromatic aberration wobble kicks in, motion blur on player.
- Time-ghost trails behind enemies (semi-transparent "where they were 0.5s ago" silhouettes).
- Audio pitch-shifts down ~5 semitones, SFX reverb increases.
- Cascades are *announced* 3 seconds early by a screen pulse and unique audio sting.

**Why this twist is interesting**
- It's a *tempo change*, not a power increase. You don't get stronger — the world gets more readable.
- Rewards pattern recognition over twitch reflexes.
- Creates a controlled-chaos feeling: you can choose to dive into the densest enemy pack for huge payouts, or play the perimeter safely.
- Mechanically trivial in Bevy: a `CascadeState` resource + a `CascadeAffected` marker component, scaled in one system.
- Pairs naturally with Megabonk mobility: when time slows, your dash i-frames feel enormous, your slides chain into slides.
- Works *with* the auto-attack scaling from Vampire Survivors — the more weapons you have, the more you can clear during Cascade.

**Implementation note**
- DON'T scale `Time` globally — that breaks the player. Instead, multiply the velocity of `CascadeAffected` entities each frame:
  ```rust
  fn cascade_scale_system(
      cascade: Res<CascadeState>,
      mut q: Query<(&BaseVelocity, &mut Velocity), With<CascadeAffected>>,
  ) {
      let s = if cascade.active { 0.25 } else { 1.0 };
      for (base, mut v) in &mut q { v.0 = base.0 * s; }
  }
  ```
  Run this *after* physics integration, *before* transform updates.

### Backup twist A — **The Lich's Ledger**
- Cross-run soul collection. Every enemy you kill persists as a "soul" in your ledger.
- Souls generate passive currency between runs.
- BUT: souls have **weight**. More souls = slower movement in future runs.
- Voluntary "purge shrines" let you burn souls for a one-run power boost.
- Creates a meta-game around greed vs. mobility.

### Backup twist B — **Cascade + Carry Decay**
- Loot in your inventory **degrades** each second. Rarer loot decays faster.
- You can extract at any time, but greed costs you.
- Adds risk to the "one more room" impulse.
- Composable with Cascade: 4× decay during Cascade = 4× greed penalty.

**Recommended**: start with **Cascade Mode** as the only twist. Adding more before playtesting risks bad balance. The backups are easy to bolt on later.

---

## 3. Core Loop (90-second micro-cycle)

```
0:00   Drop into a procedural arena (3-room "biome" cluster, 50x50 tiles)
0:15   Wave 1: 8-12 grunts, 2-3 common weapons on ground
0:30   Wave 2: 16-20 mixed, one elite
0:45   Mini-boss spawns (guaranteed rare+ drop)
1:15   Mini-boss dies → loot explosion → Cascade opens
1:45   Cascade closes → boss wave spawns
2:00   Extraction portal appears → player choice: extract or push to next biome
```

The 5-minute hard cap means you'll see **2-4 biomes per run** if you push greedily, or **1-2 biomes** with full extraction. The "did I extract at the right time?" question is the core meta-decision.

---

## 4. Combat Mechanics

### Input
| Action | Keyboard | Gamepad |
|---|---|---|
| Move | WASD | Left stick |
| Aim | Mouse | Right stick |
| Dash | Space | A / RB |
| Slide | Shift | B / RT |
| Weapon swap | 1-4 / Q-E | D-pad |
| Special ability | F | Y |
| Interact / extract | E | X |

### Movement (Megabonk-influenced)
- **Walk speed**: 220 px/s
- **Dash**: 600 px/s, 0.2s i-frames, 1.5s cooldown, in aim direction (or movement direction if no aim)
- **Slide**: 350 px/s, 0.4s i-frames, breaks line-of-sight briefly (1.0s cooldown after slide ends)
- **Wall-kick**: tap dash while sliding into a wall = perpendicular bounce, preserves i-frames
- **Coyote time**: 0.1s (still able to dash a frame after leaving a wall)

### Damage model
- **Base HP**: 4, +1 per level-up (max 9)
- **One-hit protection**: a hit that would kill you leaves you at 1 HP and grants 0.5s invuln
- **Crit**: weapons have 5-15% crit chance, 2× damage
- **Damage sources**: projectiles, contact, AoE circles, DoT zones

### Dodge feel
- Dash and slide i-frames are tuned to **just barely** dodge a single projectile — chained dodges are required for bullet-hell patterns (Enter the Gungeon baseline)
- In Cascade, those i-frames feel huge because enemy projectiles move 4× slower

### Auto-attack layer (Vampire Survivors fusion)
- Once you have 2+ weapons, **secondary fire** triggers automatically
- Auto-attacks target the nearest enemy within 60° of your aim
- Fire rate, damage, projectile speed are stat-based
- The more weapons, the more auto-attacks stack — by minute 4, expect 6-8 weapons all firing constantly

### Character abilities
- 4-6 playable characters, each with one unique ability
- Examples:
  - **The Marauder**: F = grenade arc (AOE damage)
  - **The Reaver**: F = dash-strike (lunge forward with melee damage + i-frames)
  - **The Spectre**: F = brief invisibility (1.0s, breaks on attack)
  - **The Engineer**: F = deploy turret (auto-fires for 5s)

---

## 5. Arsenal System: Modular Procedural Guns

Inspired by Enter the Gungeon's curated feel, fused with Vampire Survivors' stat-stacking and Borderlands-style part assembly.

### Each weapon = **Barrel + Magazine + Stock + Optic + Modifier**

| Slot | Options (4-6 each) | Determines |
|---|---|---|
| **Barrel** | Pistol, Shotgun, Burst, Beam, Spiral, Launcher | Fire pattern & projectile count |
| **Magazine** | Kinetic, Energy, Fire, Ice, Void, Toxic | Damage type & status effects |
| **Stock** | Light, Heavy, Marksman, Rapid, Bounce | Recoil, fire rate, projectile speed |
| **Optic** | Iron, Holo, Homing, Pierce, Chain | Aim assist, projectile pass-through |
| **Modifier** | Explosive, Lifesteal, Ricochet, Toxic, Cascade+, etc. | Special effects (1-2 per legendary) |

**Math**: 5^5 = 3,125 base combinations, ×50 modifiers = effectively infinite within rarity constraints.

### Rarity tiers
- **Common (white)**: 1 slot rolled
- **Uncommon (green)**: 2 slots rolled
- **Rare (blue)**: 3 slots rolled
- **Epic (purple)**: 4 slots rolled
- **Legendary (orange)**: 5 slots rolled, 1-2 modifiers, named ("The Voidhowl")

### Acquisition
- Drop from enemies (rarity weighted by Cascade bonus)
- Found in treasure rooms
- Bought from vendors (between biomes)
- Transmuted: 3 commons → 1 random uncommon, etc.

### Inventory
- 4 weapon slots (expandable to 6 via meta-progression)
- Swap is instant (no weapon wheel in action — D-pad flick)

---

## 6. Procedural Generation

### Biomes
- Each run = 3-5 biomes in sequence
- Each biome = 3-room cluster (arena / treasure / gauntlet / boss)
- Biomes are visually distinct but mechanically similar (theme affects enemies + hazards, not rules)

### Room types
- **Arena**: 3-5 escalating waves, exit portal
- **Treasure**: 30% fewer enemies, 2× loot density
- **Gauntlet**: timed, 60s to clear, scaling difficulty
- **Boss**: scripted mini-boss, guaranteed epic+ drop

### Tile tech
- **Recommended**: `bevy_ecs_tilemap` (or custom chunked tilemap) for ECS-friendliness
- Each room is a 50×50 grid, 16px tiles = 800×800 px
- Procedural assembly: 4-5 "room chunks" per room type, joined at doors

### Wave system
- Spawners are entities with `WaveDefinition` + `Timer` components
- Wave definitions are **deterministic per seed** (no RNG within a run) — enables async multiplayer and daily seeds
- 3-5 waves per arena, escalating enemy count and type

### Determinism
- Use `fastrand` (seeded) or `rand_xoshiro` for ALL randomness
- Particle systems use seeded randomness so they're reproducible
- This is **critical** for async multiplayer (gravestones from other players' runs must appear in expected places)

---

## 7. Meta Progression

### Persistent systems

| System | What it tracks | What it does |
|---|---|---|
| **Soul Imprint** | Kills per weapon archetype | After 50 kills with a weapon archetype, that archetype gains +10% dmg to that enemy type. Imprints persist across runs. |
| **Crown Currency** | Currency earned per run | Spent on meta-upgrades (HP, dash, weapon slots, starting items) |
| **Cascade Mastery** | Best Cascade KO count in a single run | Unlocks perks at 50/100/250/500 KO milestones |
| **Enemy Codex** | First-kill of each enemy type | Unlocks stats + lore snippet |
| **Daily Seed** | One seed per day | Leaderboard, async competition |
| **Black Market** | Cosmetic-only | Skins, weapon tints, death effects, music tracks |

### Risk vs reward
- The longer you survive, the more Crown you earn, but the harder extraction becomes
- Extracting early = safe small payout; pushing deep = risky big payout
- "Soul Imprint" creates specialization pressure: you want to keep one weapon archetype, but the game constantly tempts you to swap

---

## 8. ECS Architecture in Bevy

### Recommended Bevy version
**0.14+** (or current stable at start). Use the `bevy` crate with default features + `bevy_ecs` re-exports.

### Entity archetypes (the "what exists")

| Entity | Components | Notes |
|---|---|---|
| **Player** | `Player`, `PlayerInput`, `Health`, `Velocity`, `Collider`, `Inventory`, `Transform` | One per run |
| **Enemy** | `Enemy`, `Health`, `Velocity`, `EnemyAI`, `DamageOnTouch`, `Collider` | Variants: Grunt, Dasher, Shooter, Bomber, Tank, Sniper, Boss |
| **Projectile** | `Projectile`, `Velocity`, `Damage`, `Lifetime`, `Team` | Player or enemy variant |
| **Weapon** | `Weapon`, `WeaponStats`, `FireTimer`, `AutoTarget` | Player-side only |
| **Item** | `Item`, `ItemRarity`, `ItemStats`, `Weight` | World pickup |
| **Spawner** | `Spawner`, `WaveDefinition`, `SpawnTimer` | Per arena |
| **Particle** | `Particle`, `Lifetime`, `Velocity`, `Color` | Visual only |
| **CascadeZone** | `CascadeZone`, `Radius`, `Lifetime` | Optional area-denial variant |
| **Gravestone** | `Gravestone`, `OwnerId`, `Loot`, `Lifetime` | Async multiplayer |

### Core component sketches

```rust
#[derive(Component)]
struct Health { current: f32, max: f32 }

#[derive(Component)]
struct Velocity(Vec2);

#[derive(Component, Default)]
struct BaseVelocity(Vec2); // physical "true" velocity, not affected by Cascade

#[derive(Component)]
struct PlayerInput {
    move_dir: Vec2,
    aim_dir: Vec2,
    dash_pressed: bool,
    slide_pressed: bool,
    swap_slot: Option<u8>,
}

#[derive(Component)]
struct Weapon {
    archetype: WeaponArchetype, // Pistol, Shotgun, etc.
    damage_type: DamageType,
    stats: WeaponStats,
    fire_timer: Timer,
    auto_target: Option<Entity>,
}

#[derive(Component, Default)]
struct CascadeAffected; // marker: this entity is scaled during Cascade

#[derive(Component)]
struct Projectile {
    damage: f32,
    team: Team, // Player or Enemy
    lifetime: Timer,
    pierce: u8,
}

#[derive(Component)]
struct EnemyAI {
    behavior: AiBehavior, // Charger, Shooter, Orbit, etc.
    state: AiState,
    target: Option<Entity>,
}
```

### Resources (singletons)

```rust
#[derive(Resource, Default)]
struct CascadeState {
    active: bool,
    time_remaining: Timer,
    kills_during: u32,
    chain_extends: u32,
}

#[derive(Resource)]
struct RunState {
    seed: u64,
    biome_index: u32,
    time_elapsed: f32,
    time_cap: f32, // 300s
    cascade_beacon_alive: bool,
    score: u32,
    enemies_killed: u32,
}

#[derive(Resource)]
struct GameSettings {
    dash_iframe_ms: u32,
    cascade_scale: f32, // 0.25
    cascade_duration_s: f32,
    cascade_interval_s: f32,
}
```

### Systems (the logic)

**Ordered with explicit system sets:**

```rust
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
enum GameSet {
    Input,
    Cascade,
    AI,
    Movement,
    Collision,
    Damage,
    Weapons,
    Spawning,
    Cleanup,
    UI,
}

// In your plugin:
app.configure_sets(
    Update,
    (
        GameSet::Input,
        GameSet::Cascade,
        GameSet::AI,
        GameSet::Movement,
        GameSet::Collision,
        GameSet::Damage,
        GameSet::Weapons,
        GameSet::Spawning,
        GameSet::Cleanup,
        GameSet::UI,
    ).chain(),
);
```

| System | Set | Purpose |
|---|---|---|
| `input_system` | Input | Read keyboard/gamepad → `PlayerInput` |
| `cascade_state_system` | Cascade | Update `CascadeState.active` and timer |
| `cascade_scale_system` | Cascade | Multiply `Velocity` of `CascadeAffected` entities by cascade scale |
| `enemy_ai_system` | AI | Update `EnemyAI` state, set `Velocity` from `BaseVelocity` |
| `movement_system` | Movement | Integrate `Velocity` into `Transform` |
| `collision_system` | Collision | Projectile↔enemy, player↔enemy, AoE checks |
| `damage_system` | Damage | Apply damage, fire death events |
| `weapon_fire_system` | Weapons | Tick `FireTimer`, spawn projectiles |
| `auto_aim_system` | Weapons | Assign `AutoTarget` to auto-fire weapons |
| `wave_spawn_system` | Spawning | Tick `SpawnTimer`, instantiate enemies from `WaveDefinition` |
| `cleanup_system` | Cleanup | Despawn dead `Lifetime` components |
| `ui_update_system` | UI | Refresh HUD, weapon wheel, Cascade timer |

### Recommended plugins
- **`bevy_xpbd_2d`** — physics & collision (purely Rust, ECS-native, fast)
- **`bevy_ecs_tilemap`** — procedural tilemaps
- **`bevy_kira_audio`** — audio with time-pitching for Cascade
- **`bevy_hanabi`** — GPU particle system (Cascade ghosts, explosions)
- **`bevy_gizmos`** — debug visualizations
- **`bevy_asset_loader`** — runtime asset loading
- **`bevy_egui`** — dev menus, debug UI
- **`bevy_state` + custom sub-states** — `Loading`, `MainMenu`, `InRun`, `Paused`, `GameOver`

### Key ECS patterns

**Spatial query for dense crowds**
With 200+ entities, naive O(n²) collision is dead. Use:
- `bevy_xpbd_2d`'s spatial query for physics
- Or implement a uniform grid (10×10 cells) — each frame, bin entities, query neighbors

**Event channels for decoupling**
- `DeathEvent`, `DamageEvent`, `CascadeStartEvent`, `CascadeEndEvent`, `LootDropEvent`
- Decouples systems, enables easy debugging/observability

**Component bundles for entity factories**
```rust
#[derive(Bundle)]
struct EnemyBundle {
    enemy: Enemy,
    health: Health,
    velocity: Velocity,
    base_velocity: BaseVelocity,
    ai: EnemyAI,
    cascade: CascadeAffected,
    collider: Collider,
    sprite: SpriteSheetBundle,
}
```

**Determinism for async multiplayer**
- All RNG goes through one `Res<RunRng>` resource
- Particle systems read from `RunRng` not `rand::thread_rng()`
- `Float` ordering matters: use `f32` arithmetic in consistent order

---

## 9. Risk Areas / Open Questions

| Risk | Mitigation |
|---|---|
| **Time scaling jitter** with 500+ entities | Only scale `Velocity` once per frame in `cascade_scale_system`; benchmark early |
| **Auto-aim thrashing** when 50+ enemies clustered | Use hysteresis on target selection; only swap target if current target dies or leaves cone |
| **Cascade visual readability** — 4× damage can one-shot | Strong directional damage indicators, screen shake, audio sting on hit; one-hit protection as safety net |
| **Determinism bugs** in async multiplayer | Strict `RunRng` discipline, snapshot tests, replay comparison tool |
| **Balance** of 5-min cap + Cascade timing | Playtest loop: ship a 1-weapon, 1-enemy prototype in 2 weeks, balance from there |
| **Content volume** — procedural helps but enemies need variety | Start with 4 enemy types + 1 mini-boss per biome; content > systems for first playtest |
| **Solo vs async multiplayer** | Phase 0 is solo-only; async multiplayer is a stretch goal (Phase 5) |

---

## 10. Development Phases

### Phase 0 — Combat Feel (2-3 weeks)
- Bevy 0.14 project scaffold
- Player: WASD move, mouse aim, space dash
- 1 weapon (pistol), 1 enemy (chaser)
- 1 small arena, 1 wave
- NO Cascade, NO procgen, NO meta
- **Goal**: validate core "feel" — does it feel like Nuclear Throne?

### Phase 1 — Combat Vertical Slice (4-6 weeks)
- 4 weapons, 4 enemy types, 1 mini-boss
- Auto-attack system working
- Cascade Mode prototype
- Particle effects (bevy_hanabi)
- Damage numbers, screen shake
- **Goal**: Cascade Mode feels awesome

### Phase 2 — Run Structure (4-6 weeks)
- Procedural arena (1 biome, 3 rooms)
- Wave spawning system (deterministic per seed)
- Loot drops, weapon pickups
- Extraction mechanic, 5-min hard cap
- **Goal**: a complete if short run (2 biomes)

### Phase 3 — Meta Progression (2-3 weeks)
- Soul Imprint system
- Crown currency + meta-upgrade tree
- Save/load
- 2-3 biomes
- 2-3 playable characters
- **Goal**: replayability & identity

### Phase 4 — Polish (4-8 weeks)
- Audio + SFX + adaptive music
- UI/UX pass
- Tutorial / first-run onboarding
- Visual juice (hitstop, particles, screen transitions)
- Balance pass with playtesters
- **Goal**: shippable demo

### Phase 5 — Stretch Features (4+ weeks)
- Async multiplayer (gravestones, contract pool, daily seed)
- Leaderboards
- Daily challenge mode
- **Goal**: community features

**Total: ~20-30 weeks** for a polished solo-mode demo. Async multiplayer is a separate 4-week chunk after that.

---

## 11. Next Steps — Decision Points

Before I scaffold anything, I need your call on:

1. **Twist**: Cascade Mode (my recommendation) vs. Lich's Ledger vs. Carry Decay?
2. ~~**Visual style**: pixel art (Gungeon-style), vector (Vampire Survivors), painterly?~~ → **DECIDED 2026-06-05: defer to Phase 1+**. Keep colored squares for Phase 0/1, pick style once we know what the game feels like. Sound design decision: validate feel before committing to asset pipeline.
3. **Phase 0 scope confirmation**: 2-3 weeks for combat feel — is that your target, or do you want to skip ahead?
4. **Tooling**: rust-analyzer + Bevy CLI + a tilemap editor? I'll write a `setup.md` with the dev environment if you say go.
5. **Solo only or solo-first**: confirming async multiplayer is Phase 5 stretch, not Phase 0.

Once you answer those, I'll generate:
- A `setup.md` with the dev environment, recommended Cargo.toml, and project structure
- A `phase_0_prototype.md` with a step-by-step Bevy tutorial tailored to RUSTWORN's first slice
- A `data_model.md` if you want to lock down the ECS schema before coding

---

## Appendix A — Why this combo works

The four reference games occupy a design space:

```
         Manual Aim
              ↑
              |
  Nuclear     |    Enter the
  Throne      |    Gungeon
  (twin-stick)|    (bullet-hell)
              |
  ←———————+————+———————→
  Simple  |    |  Curated
  Weap.   |    |  Arsenal
          |    |
  Vampire |    |   (this is
  Surviv. |    |    the
  (auto)  |    |    gap)
          ↓    ↓
       Auto Attack
```

RUSTWORN sits in the **upper-right** quadrant: manual aim + curated arsenal, with an **auto-attack layer** (Vampire Survivors fusion) that kicks in once you have 2+ weapons. The Cascade Mode twist adds a *temporal* axis that none of the references use — it's a tempo change, not a power increase.

## Appendix B — Why Bevy ECS is well-suited

The reference games use various engines, but the ECS approach fits RUSTWORN's profile particularly well:

- **Data-oriented by design** — auto-attack weapons, projectile pools, and enemy swarms are all entity collections
- **System ordering is explicit** — input → AI → movement → collision is naturally a system chain
- **Component composition** — modular weapons = `Barrel` + `Magazine` + ... as components
- **Resource pattern for state** — Cascade state, run state, settings are all resources
- **Hot-reload friendly** — Bevy's asset system makes iteration fast

Compared to Godot (Node-based) or Unity (MonoBehaviour-based), ECS is overkill for *some* games but exactly right for this one.
