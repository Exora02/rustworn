# Phase 0 — Combat Feel Prototype

> **Goal**: prove out the core "feel" of combat in 2-3 weeks.
> **Acceptance**: you can run the game locally, walk around with WASD, aim with the mouse, dash with Space, fire a projectile, and kill a chaser enemy. No art yet — colored shapes.

This is the slice that answers the most important question of the whole project: **does it feel like Nuclear Throne?**

---

## 1. Scope — what to build

| Feature | System(s) | Plugin |
|---|---|---|
| Player WASD movement | `player_movement_system` | `PlayerPlugin` |
| Mouse aim (independent of movement) | `player_aim_system` | `PlayerPlugin` |
| Space-bar dash with i-frames | `dash_system`, `iframe_system` | `PlayerPlugin` |
| Single pistol (spawned entity) | `weapon_fire_system` | `CombatPlugin` |
| Click-to-fire projectile | `weapon_fire_system` | `CombatPlugin` |
| Projectile lifetime + cleanup | `projectile_lifetime_system` | `CombatPlugin` |
| One chaser enemy that spawns | `enemy_ai_system` | `EnemyPlugin` |
| Enemy takes damage, dies, drops nothing | `damage_system`, `death_system` | `CombatPlugin` |
| Camera follows player | `camera_follow_system` | `PlayerPlugin` |
| Basic HUD: HP + ammo | `hud_update_system` | `UiPlugin` |

## 2. Scope — explicitly NOT in Phase 0

These are tempting to add but should wait:

- ❌ Multiple weapons / weapon swap
- ❌ Multiple enemy types
- ❌ Cascade Mode
- ❌ Procedural rooms
- ❌ Wave spawning
- ❌ Loot drops
- ❌ Sound
- ❌ Particles
- ❌ Boss enemies
- ❌ Meta progression
- ❌ Save/load
- ❌ Polish (screen shake, hitstop, juice)

**Discipline**: when you feel the urge to add Cascade Mode because "the game feels empty", don't. Add a second enemy type instead. Cascade lands in Phase 1, after you have enough stuff to test it against.

---

## 3. Implementation order

Suggested 2-3 week timeline. Each step should leave the game runnable.

### Day 1-2: Player movement
- WASD input → `Velocity`
- `BaseVelocity` updated from input
- Movement system applies `Velocity` to `Transform`
- Camera follows player with damping

**Acceptance**: a square moves around the screen with WASD. Camera follows. Nothing else.

### Day 3-4: Mouse aim + dash
- Mouse position → `aim_direction` resource
- Player sprite flips to face aim direction (or use a child entity for the gun)
- Space-bar → trigger dash
- Dash: spawn 0.2s timer, set `Velocity` to dash_speed × aim_dir, mark player as `Invulnerable`
- `Invulnerable` component ticks down; while active, contact damage is ignored

**Acceptance**: player can move + aim. Space makes them lunge in the aim direction with brief i-frames. Visual feedback: maybe a color flash.

### Day 5-7: One weapon, one projectile
- Add a `Weapon` component on the player (just one for now)
- `weapon_fire_system` ticks a `FireTimer`; on fire, spawn a `Projectile` entity at the player's position
- Projectile has `Velocity` and `Lifetime` (2s)
- Cleanup system removes `Projectile` when `Lifetime` elapses
- Click (left mouse) → trigger fire
- `auto_aim_system` is NOT yet in scope (manual aim only)

**Acceptance**: click to fire a projectile that travels in the aim direction and despawns after 2s.

### Day 8-10: One enemy type
- Add a `Spawner` entity that despawns after 5s
- It spawns one chaser enemy at a random edge of the screen
- Chaser has `EnemyAI::Chaser` behavior, `BaseVelocity` pointing at player
- On contact with player, deal 1 damage, knock back

**Acceptance**: a single chaser spawns and chases the player. Contact damage works.

### Day 11-12: Damage + death
- Projectile → enemy collision check (simple AABB)
- On hit: enemy takes 1 damage, projectile despawns
- Enemy at 0 HP: spawn a tiny "death puff" (just a fading colored square for now), despawn enemy
- Player at 0 HP: print "you died" and exit (real game over comes in Phase 2)

**Acceptance**: shoot a chaser 3 times, it dies. Player gets hit 4 times, game exits.

### Day 13-14: HUD + minimal juice
- HP bar in top-left (4 hearts or a rectangle)
- Fire-rate indicator (just the timer progress bar)
- Screen shake on player damage
- Color flash on hit
- Hitstop (0.05s freeze) on enemy death

**Acceptance**: when you kill an enemy or take damage, the game *feels* responsive. The screen shakes, the enemy flashes, there's a brief freeze.

### Day 15-16: Polish pass
- Tuning: dash speed, fire rate, enemy speed, damage values
- Cap projectile count (don't let the player lag the game with infinite bullets)
- Add a small "wave timer" so the spawner auto-spawns a new enemy every 10s

**Acceptance**: the prototype is fun to play for 2-3 minutes at a time. You can kill 10+ enemies, dash through bullets, etc.

---

## 4. New components to add

Append to `src/components.rs` as you go:

```rust
#[derive(Component)]
pub struct Invulnerable(pub Timer);

#[derive(Component)]
pub struct Enemy;

#[derive(Component, Default)]
pub struct EnemyAI {
    pub behavior: AiBehavior,
    pub target: Option<Entity>,
}

#[derive(Default, Clone, Copy)]
pub enum AiBehavior {
    #[default]
    Chaser,
    // future: Shooter, Orbit, Sniper, ...

#[derive(Component)]
pub struct Projectile {
    pub damage: f32,
    pub team: Team,
    pub lifetime: Timer,
}

#[derive(Default, Clone, Copy)]
pub enum Team { Player, Enemy }

#[derive(Component)]
pub struct Weapon {
    pub archetype: WeaponArchetype,
    pub damage: f32,
    pub fire_rate: f32, // shots per second
    pub projectile_speed: f32,
    pub fire_timer: Timer,
}

#[derive(Default, Clone, Copy)]
pub enum WeaponArchetype { Pistol }  // expand later
```

## 5. New plugins to add

```
src/plugins/
├── mod.rs            # export them all
├── setup.rs          # already exists
├── player.rs         # PlayerPlugin
├── combat.rs         # CombatPlugin (weapons, projectiles, damage)
├── enemy.rs          # EnemyPlugin (spawners, AI)
├── camera.rs         # CameraPlugin (follows player)
└── ui.rs             # UiPlugin (HUD)
```

Each plugin follows the same shape as `SetupPlugin`:

```rust
pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            player_input_system,
            player_movement_system,
            player_aim_system,
            dash_system,
            iframe_tick_system,
        ).chain());
    }
}
```

## 6. System ordering

Use Bevy's `SystemSet` for explicit ordering:

```rust
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
enum GameSet {
    Input,      // read keyboard/mouse
    Cascade,    // (Phase 1) scale cascade-affected velocities
    AI,         // update enemy state, set BaseVelocity
    Movement,   // apply BaseVelocity to Transform (after Cascade in Phase 1)
    Collision,  // AABB checks
    Damage,     // apply damage, queue death events
    Weapons,    // fire weapons, tick fire timers
    Spawning,   // spawn enemies, projectiles
    Cleanup,    // despawn dead entities
    UI,         // update HUD
}
```

Configure them in `lib.rs`:

```rust
app.configure_sets(
    Update,
    (GameSet::Input, GameSet::Cascade, GameSet::AI,
     GameSet::Movement, GameSet::Collision, GameSet::Damage,
     GameSet::Weapons, GameSet::Spawning, GameSet::Cleanup,
     GameSet::UI).chain(),
);
```

This matches the long-term architecture in PLAN.md §8.

## 7. Acceptance test

The Phase 0 prototype is done when:

- [ ] Game launches on local Windows with no errors
- [ ] Player moves with WASD, aims with mouse, dashes with Space
- [ ] Player fires a projectile on left-click
- [ ] One chaser enemy spawns, follows player, deals contact damage
- [ ] 3 hits to kill an enemy, 4 hits kills the player
- [ ] HUD shows HP
- [ ] Screen shakes on hit
- [ ] Game feels *good* to play for 2-3 minutes straight
- [ ] `cargo check` passes on VPS with zero new warnings
- [ ] `cargo clippy` passes
- [ ] All changes pushed to Forgejo

When all those boxes are checked, you're ready for Phase 1.

---

## 8. If you get stuck

- **Bevy doesn't feel right** — try the official [Bevy Breakout tutorial](https://bevyengine.org/learn/book/tutorials/) or the [Bevy New 2D](https://github.com/TheBevyFlock/bevy_new_2d) template for comparison.
- **ECS is confusing** — read the [Bevy ECS book chapter](https://bevyengine.org/learn/book/getting-started/ecs/) and the official examples.
- **Performance issues** — Bevy is fast. If you have perf problems, the answer is almost always "you have too many entities" or "your collision is O(n²)". Don't reach for optimizations until you have 1000+ entities.
- **It doesn't feel like Nuclear Throne** — play more Nuclear Throne. The trick is: short i-frames, fast movement, weapons that punch.
- **Art** — at this phase, just use colored squares and rectangles. Don't waste time on art until combat feels right.
