# Rustworn Strategic Vision

> **Why this game exists** — identity, scaling, bosses, and the roadmap to "must-play"

---

## Part A: Rustworn's Identity (The Hook)

The game currently plays as a competent Gungeon/Isaac/VS fusion with solid bones. Its problem: **nothing makes it special**. Cascade is a cool tempo mechanic, curse is a risk/reward lever, and extraction *was* a meta-loop (now removed in favor of infinite runs). But none of these are *remembered*. When someone finishes Isaac they remember Brimstone + technology lasers. When someone finishes Gungeon they remember the Bullet That Can Kill The Past. What does Rustworn leave them with?

**The answer — two game-defining mechanics built on existing systems.**

---

### Hook #1: Rust Accumulation (Thematic Scaling)

**Core idea**: The world doesn't "get harder" — it *corrodes*. Every room you clear leaves a mark on your run. Enemies that survive the room are harder to kill *next time*. The environment decays around you. The run ends not because the numbers got too big, but because the world has *rusted shut*.

**How it works (building on existing WaveSpawner, RunState, and StatusEffects):**

1. **Per-room rust layer**: Each cleared room applies a `RustLevel` to the floor. This isn't "enemies get +HP" — it's a tangible, visible affliction:
   - **Rust Slick**: Floor tiles in older rooms become slippery (player and enemies slide slightly on movement). Mechanically: a `RustFloor` component on the room's physics that applies a small lateral drift force to entities moving across it.
   - **Corroded Walls**: After room 8+, wall edges develop "rust cracks" that periodically emit slow-moving corrosion clouds (existing `Projectile`, Team::Enemy, 0.5 damage, slow speed).
   - **Weapon Rust**: Weapons that you don't swap out for 2+ rooms lose 5% damage (cumulative). Swapping a weapon "wipes its rust" as the action of holstering knocks the corrosion loose. *This creates a built-in reason to cycle through your 4 weapon slots instead of finding one god-gun and holding it forever.*

2. **Visual language**: The rust isn't just stat bars — it's visible. The room's wall color (already biome-themed) shifts toward brown/orange as rust accumulates. Enemies have a faint orange overlay at high rust levels (reusing the existing `BaseColor` component and tinting system). The Cascade telegraph overlay becomes *corroded* rather than purple — rust-colored crack lines.

3. **Why this is the hook**:  
   - It's *thematic* — the game is called RUSTWORN. Rust should be the protagonist.  
   - It creates *interesting decisions*: do I keep my god-gun and let it rust, or swap to a weaker gun to clean it? Do I push through a high-rust room or backtrack to a clean one?  
   - It's *visible, tactile, and obeys the show-don't-tell rule*. Players will *see* the rust spreading.  
   - It *replaces* the boring "enemies get +HP/+damage" scaling with something that changes *how the game plays*, not just the numbers.

---

### Hook #2: Cascade Echoes (Time-Anomaly System)

**Core idea**: Every Cascade window leaves a scar in the timeline. The *next* time you enter a room that was active during a Cascade, "time echoes" of that Cascade persist — creating zones of altered time that the player and enemies both interact with.

**How it works (building on existing CascadeState, RoomScope, and the cascade timing system):**

1. **When a Cascade ends**, the game records the room IDs that had active enemies during that Cascade. These rooms become "Echo-touched" — they gain residual time-anomaly zones:
   - **Slow zones**: Patches of the floor where enemies move at 0.5× speed (good for the player) but player projectiles also slow down (bad).
   - **Speed zones**: Patches where enemies move at 2× speed but the player's dodge i-frames last longer.
   - **Double-hit zones**: In the exact spot where the Cascade Beacon died, a "time-echo" persists — any entity entering the zone takes damage again from the echo of the Beacon's death. This creates interesting kiting geometry.

2. **Cascade synergy with Rust**: When a Cascade Echo overlaps with a Rust Slick, the interaction creates a "Void Crack" — a temporary floor hazard that damages both player and enemies. This rewards players who think spatially: "I'll trigger Cascade in a high-rust room to create a kill zone."

3. **Why this is the hook**:  
   - It makes Cascade *meaningful after it ends*, not just "30 seconds of bullet time, then back to normal."  
   - It ties Cascade to the spatial memory of the run — rooms you fought in during Cascade are *different forever*.  
   - It rewards tactical Cascade timing (do you Cascade in a hard room to clear it, or in an easy room to create safe Echoes for later?).  
   - It gives Cascade a *cost* beyond cooldown management — every Cascade you trigger shapes the remaining run.

---

### Combined Identity Statement

> **Rustworn is a roguelike about *decay and memory*. The world rusts around you as you push deeper, and time itself leaves scars. Every Cascade window changes the rooms you fought in forever. Your weapons corrode if you hold them too long. The run doesn't end because the numbers got too big — it ends because the world has rusted shut around you, and the last room is a collapsing cage of corrosion and frozen time.**

This is what someone remembers. This is the Steam description. This is the hook.

---

## Part B: Enemy Scaling — Rust Accumulation System

### The Problem With Existing Scaling

The current system (Phase 2.3.2): `per-room ×1.08 HP / ×1.05 dmg`. This is placeholder math — it works, but it's invisible. The player can't *feel* that an enemy has 8% more HP. They just notice that rooms take longer to clear. That's not tension, it's tedium.

### The Solution: Rust Accumulation

**Replace** the invisible `×1.08`/`×1.05` multipliers with **visible, systemic** scaling that changes *how the game plays*.

### Implementation Plan

#### Data Model

Add to `src/resources/state.rs`:

```rust
/// Per-run rust level. Starts at 0, increments on room clear.
/// Caps at 10 (beyond that, the run is essentially Rustmageddon).
#[derive(Resource)]
pub struct RustLevel {
    pub current: u32,        // 0..10
    pub per_room_increment: u32, // +1 per cleared room by default
}
```

Add to `src/components/world.rs`:

```rust
/// Applied to a room's physics collider after the room is cleared.
/// Generates rust effects for future visits.
#[derive(Component)]
pub struct RustedRoom {
    pub level: u32,
    pub slick_intensity: f32,       // 0.0..1.0, controls slide force
    pub wall_corrosion: Timer,      // ticks corrosion cloud spawn
    pub cracks_open: bool,          // true when rust >= 5
}
```

Add to `src/components/enemy.rs`:

```rust
/// Applied to enemies in high-rust rooms. Doesn't change their HP —
/// changes their behavior.
#[derive(Component)]
pub struct RustInfected {
    pub level: u32,
    /// Emits a slow-moving corrosion cloud on a timer
    pub emission_timer: Timer,
}
```

Add to `src/components/player.rs`:

```rust
/// Tracks rust on each weapon slot. Cleared on weapon swap.
#[derive(Component)]
pub struct WeaponRust {
    pub per_slot: [f32; 4],  // 0.0..1.0, each = -10% damage at max
    pub room_counter: [u32; 4], // rooms held without swap
}
```

#### Scaling Curve (instead of ×1.08/×1.05)

| Rust Level | Effect | Visible Cue |
|-----------|--------|-------------|
| 0-1 | None | Clean floor tiles |
| 2-3 | Rust Slick: 10% slide force on movement | Floor tiles have orange specks |
| 4-5 | + Corrosion clouds: walls emit slow poison clouds every 8s | Wall edges glow orange |
| 6-7 | + Weapon Rust: weapons held 3+ rooms lose 5% dmg per room | Weapon sprite gains orange tint |
| 8-9 | + Enemies spawn with `RustInfected`: emit corrosion clouds on contact | Enemies have orange overlay |
| 10 | + Void Cracks: floor hazards appear between rusted zones | Black/orange crack lines on floor |

**Enemy HP scaling is removed entirely.** Enemies have the same HP at room 1 and room 20. What changes is the *environmental pressure* — the room itself fights against you.

#### Tactical Implications

- **Fast room clearers** (Vampire Survivors-style AoE builds) feel less rust pressure because rooms are cleared before the slick accumulates.
- **Kite-and-survive players** feel rust harder because they spend more time in each room.
- **Weapon hoarders** (holding one weapon for 5 rooms) pay a damage penalty. **Swappers** (cycling all 4 slots) keep weapons clean. This creates build diversity.
- **Curse synergy**: Each curse point adds +1 to the effective Rust Level for loot calculations (higher curse = better loot but at higher rust levels). Already wired in the director credit system — just hook the Rust Level read there.

#### Implementation Steps

1. Add `RustLevel` resource, increment on RoomClearedEvent
2. Add `RustedRoom` component, attach to room on clear
3. Add `RustInfected` variant to enemy spawning in rust >= 6 rooms
4. Add `WeaponRust` component to player, tick on room transitions, apply damage penalty on weapon fire
5. Visual: tint wall sprites per rust level (reuse existing biome_accent function with lerp toward orange)
6. Hook curse into effective rust level for loot calculation
7. Remove the ×1.08/×1.05 HP/damage scaling from director credit system

---

## Part C: Boss Designs

### Design Philosophy

Current bosses (Lich, Yeti, Inferno, Treant, Void Walker) are serviceable — they have phase 1 → phase 2 transitions with attack timers. The problem: they're stat-creatures. You dodge their pattern for 30 seconds, shoot them until they die, and move on. **None of them interact with existing systems** (Cascade, curse, weapon swap, rust).

New bosses should:
1. Interact with an existing system (Cascade, curse, rust, extraction memory)
2. Have a clearly readable tell — you can learn to beat them without taking damage
3. Change the *room geometry*, not just the attack pattern
4. Be visually distinct enough for a sprite artist to sketch from text

---

### Catacombs

**Current**: Lich King (3-burst spread → Phase 2: summon chasers)

#### New Boss: The Cinder-King

**Visual theme**: A charred, throne-bound king, fused to his seat by centuries of heat and decay. His crown is a ring of smoldering bone. His body is half-ash, half-ember — when he moves, pieces of him flake off and burn on the floor. The throne drags behind him, carving a trench in the stone.

**Room setup**: Catacombs-style chamber with 4 stone pillars. The floor has 2 ancient braziers that can be reignited by the Cinder-King's fire trails.

**Attack Pattern 1 — Fire Trail** (all HP levels):
- The Cinder-King drags himself across the room, leaving a 3-wide trail of burning embers on the floor.
- Embers last 6 seconds and deal 0.5 damage per tick if the player stands on them.
- **Tell**: The Cinder-King's glow intensifies for 0.5s before he moves. The throne's shadow elongates in the direction he'll drag.
- **Counter**: The player can dash across the trail (i-frames), or kite the Cinder-King into *his own trail* — touching fire stuns him for 1s (window for free damage).

**Attack Pattern 2 — Skeletal Grasp** (all HP levels):
- Every 4s, 3 skeletal hands erupt from the floor at random positions in the room, remaining for 3s.
- Hands grab any entity that touches them (player: rooted for 0.5s; enemies: held in place for 1.5s).
- **Synergy with Cascade**: During Cascade, hands grab for 2× duration. But the Cinder-King also moves slower, making it easier to land hits.
- **Counter**: The hands have low HP (1 hit). The player can shoot them preemptively, especially with the Shotgun (spread clears multiple hands).

**Attack Pattern 3 — Cascade Memory** (Phase 2, ≤50% HP):
- The Cinder-King roars, and the room's Cascade Echo history (if any) activates — creating 3 time-anomaly zones on the floor.
- These zones cycle between slow (0.25×) and fast (2×) every 3 seconds.
- The Cinder-King teleports between zones, attacking faster inside speed zones.
- **Synergy with existing system**: This pattern ONLY appears if the player triggered a Cascade in a previous room of this run. If this is the first Cascade-less run, the Cinder-King doesn't use this attack — rewarding Cascade use.
- **Counter**: Lead the Cinder-King into a slow zone, then unload your weapon swap (clean weapon, no rust penalty) while he's slowed.

**Phase 2 transition**: At 50% HP, the Cinder-King's crown explodes into 4 orbiting skulls (reuse SkullSwarm behavior). Each skull circles him and fires a slow fireball at the player every 2s. The skulls have 2 HP each. Killing all 4 skulls leaves the Cinder-King vulnerable for 3s (increased damage taken).

---

### Frost

**Current**: Frost Queen / Yeti (charge → Phase 2: ice ring)

#### New Boss: The Hail

**Visual theme**: A sentient snowball — think a rolling boulder of packed ice and debris. It's perfectly spherical, 3× the player's size, with a glowing blue core visible through cracks in the ice. As it rolls, it absorbs loose ice and small enemies from the floor, growing larger. Its "face" is a pattern of cracks in the ice that shift as it moves.

**Room setup**: A circular arena (no corners to hide in) with 4 ice pillars. The floor is slippery everywhere — every entity slides slightly on movement, including the Hail itself (but its mass makes it slide less).

**Attack Pattern 1 — Rolling Charge** (all HP levels):
- The Hail locks onto the player and begins rolling toward them, accelerating over 2s. Contact with the Hail deals 2 damage and knocks the player back.
- The Hail doesn't stop at the wall — it ricochets off walls with 80% of its incoming speed, changing direction.
- **Tell**: The blue core flashes rapidly. A wind-up sound (crunching ice) plays for 1.5s before the charge.
- **Counter**: Dash perpendicular to the charge (not away — the Hail catches up). The Hail ricochets 3 times before stopping; after the 3rd ricochet it's stationary for 2s (damage window).

**Attack Pattern 2 — Absorb and Split** (Phase 2, ≤50% HP):
- When the Hail reaches half HP, it stops and absorbs all frozen entities in the room (ice golems, snow drifters, penguin bombers — if any survived from previous waves). For each absorbed enemy, the Hail grows 10% larger and spawns 1 mini-Hail on split.
- The Hail then splits into 2 smaller versions (each 60% the size, 50% HP).
- Each mini-Hail rolls independently, using Pattern 1 but with smaller hitboxes and lower damage (1 damage each).
- Mini-Hails can re-merge if they're within 50px of each other for 3s — creating a mid-size Hail again (70% original size, 70% HP).
- **Counter**: Keep the mini-Hails separated by kiting them to opposite sides of the room. The ice pillars can block their path temporarily. If you're playing a shotgun build, you can push one mini-Hail into the other to force a premature merge (the merge animation is 1s of vulnerability).

**Attack Pattern 2.5 — Blizzard Eyes** (only if Cascade active):
- If Cascade triggers while The Hail is alive, it enters a frenzy: its rolling speed doubles, but it becomes *predictable* — it always targets the position the player was at 0.5s ago (creating a trail-following pattern the player can exploit).
- During Cascade, the mini-Hails DON'T merge — they're moving too fast to coalesce. This makes Cascade a *safe* window to fight the split boss.

**Phase 2 transition**: At 50% HP (before split), the Hail briefly opens its core, revealing a frozen heart. The heart flashes for 2s — during this window, the Hail takes 2× damage. This is the game's way of saying "unload now." After the heart closes, the split triggers immediately.

---

### Magma

**Current**: Magma King / Inferno (lob explosive → Phase 2: ground pound)

#### New Boss: The Furnace

**Visual theme**: A massive, immobile forge structure in the center of the room. It's a cube of black iron, 2.5× the player's size, with a grate on top that glows orange/red. Chains hang from the ceiling, ending in hooks that drag along the floor. The Furnace has no movement — it's a stationary boss. Its "body" is the forge; its "arms" are the chains.

**Room setup**: The Furnace is centered. The room has 4 heat vents at the cardinal points. When the Furnace vents (Pattern 2), the closest vent to the player erupts. The floor is crisscrossed with chain grooves — the Furnace's chains travel along these grooves.

**Attack Pattern 1 — Chain Sweep** (all HP levels):
- The Furnace swings a chain in a 180° arc. The chain deals 1 damage and PULLS the player half the distance to the Furnace.
- After the sweep, the chain retracts for 1.5s (window to attack).
- **Tell**: The chain on the side being swung raises up (visible elevation of the sprite) for 0.5s. The grate glows brighter.
- **Counter**: Stay on the OPPOSITE side of the furnace from the raised chain. The chain can't reach behind the Furnace. This creates a positioning game: the player wants to stay behind the Furnace, but the fire elementals (Pattern 2) spawn there.

**Attack Pattern 2 — Heat Vent** (every 6s, all HP levels):
- The Furnace glows white-hot for 1s. Then the heat vent closest to the player erupts, dealing 1 damage and applying a 0.5s stun in a 100px radius.
- **Counter**: The player can see which vent is about to erupt (it glows brighter 0.5s before). Dash away from that vent. The stun is short enough that the player can recover before the chain sweep follows up.

**Attack Pattern 3 — Forge Elementals** (Phase 2, ≤50% HP):
- The Furnace opens its grate and spawns 2 Fire Mages (reuse existing FireMage archetype) every 5s. These are smaller than standard Fire Mages and have 50% HP.
- The Fire Mages protect the Furnace — they teleport erratically and fire 3 fireballs each.
- **Cascade synergy**: During Cascade, the Fire Mages move at 0.25× speed. But the Furnace spawns them TWICE as fast (every 2.5s). This creates a "wave defense" feel during Cascade — the room fills up but the enemies are manageable.
- **Counter**: AoE weapons (Rocket Launcher, Shotgun) clear swarm elementals efficiently. Beam weapon hits both the elemental and the Furnace in one sweep. Don't save your Cascade for the Furnace unless you have an AoE build.

**Phase 2 transition**: At 50% HP, the Furnace's grate opens with a loud metallic groan. The room's ambient temperature rises (visual: screen heat-shimmer effect). The chain sweep becomes 2 chains (left and right, alternating) and the vent pattern becomes 2 vents simultaneously. The Furnace stops spawning elementals *during the transition* (3s window of respite) then resumes with Pattern 3.

---

### Verdant

**Current**: Treant Queen (heal + orbit → Phase 2: root)

#### New Boss: The Bloom

**Visual theme**: A gigantic carnivorous flower, twice the player's height, with a central "mouth" that opens and closes rhythmically. The flower is deep crimson with iridescent purple edges. Its stem is thick and thorny, embedded in the floor. The Bloom has 4 "pods" at the cardinal points — these are its reproductive organs. They inflate and deflate as The Bloom charges attacks. When the pods burst, they release clouds of pollen.

**Room setup**: A circular clearing in the Verdant Thicket. The clearing is surrounded by a ring of smaller flowers (decorative, but they rustle when The Bloom attacks, giving a visual cue). The floor is covered in roots that pulse with a faint green glow.

**Attack Pattern 1 — Pollen Cloud** (every 4s, all HP levels):
- One of the 4 pods inflates (grows to 1.5× size over 1s), then bursts, releasing a cloud of golden pollen.
- The cloud drifts slowly toward the player over 3s. If the player touches it:
  - **Confusion**: aim direction is reversed for 1.5s (left becomes right, up becomes down)
  - **Poison**: 0.5 damage per tick for 3s (1.5 total)
- **Tell**: Which pod inflates determines the cloud's initial direction. The pod glows bright gold while inflating.
- **Counter**: The cloud is slow — the player can outrun it. Dashing THROUGH the cloud gives 0.5s of confusion only (dash i-frames reduce the effect). The player can also shoot the inflating pod to pop it early (the cloud goes in a different direction and is smaller).

**Attack Pattern 2 — Root Grasp** (every 5s, Phase 2):
- The Bloom slams its stem, sending a visible root line along the floor toward the player. When the root reaches the player, it erupts into a cluster of vines that root the player for 1s.
- During rooting, a vine circle appears on the player. The circle shrinks over 1s — if the player doesn't kill the vines (3 HP, appear as 3 small sprites), they take 1.5 damage.
- **Counter**: Shoot the root line as it travels — it has 5 HP and dies in one hit from most weapons. The root line is visible for 0.5s before it reaches the player. The Bloom also roots itself momentarily during this attack (window for damage).

**Attack Pattern 3 — Bloom Cascade** (only if Cascade active):
- If Cascade triggers while The Bloom is alive, The Bloom enters an *overdrive* state: all 4 pods inflate simultaneously and release a massive cloud that covers 50% of the room.
- However, The Bloom's mouth opens wide during overdrive — the core is exposed and takes 3× damage.
- **Synergy with existing system**: This turns Cascade from "safe time" into a *greed check* — do you dash through the pollen cloud to hit the exposed core, or stay safe and wait for the cloud to dissipate?
- **Counter**: Weapons with beam or pierce hit the core through the pollen. The Rocket Launcher's explosion clears a section of cloud. If you have the Glass Cannon + Vampire Fangs synergy (DoubleEdge), you can risk the cloud for massive damage.

**Phase 2 transition**: At 50% HP, The Bloom's mouth opens permanently (the core is always exposed). The pods cycle faster (every 3s instead of 4s). The roots become more aggressive — 2 root lines simultaneously. The Bloom's stem changes color from green to dark purple (indicating corrupted life — thematic tie to the Rust system).

---

### Void

**Current**: Void Lord / Void Walker (teleport → Phase 2: void rift)

#### New Boss: The Singularity

**Visual theme**: A gravitational anomaly — a sphere of pure black, 2× the player's size, surrounded by a lensing ring of blue-white light. The Singularity doesn't have a "body" — it's a vortex. Particles of light, debris, and enemy remains orbit it constantly. The lensing ring pulses with the rhythm of gravitational waves. The room around it warps — walls appear to curve toward it.

**Room setup**: The Singularity is at the center. The room is square, but the Singularity's gravity field warps projectile paths. The floor has 4 "gravity anchors" at the corners — these are the only places the player can stand without being pulled toward the center.

**Attack Pattern 1 — Gravitational Pull** (always active):
- The Singularity constantly exerts a pull force on the player: 50 px/s² toward the center.
- The force increases by 10 px/s² per room cleared (the Rust scaling tie-in).
- At Rust level 5+, the pull is strong enough that the player can't stand still — they must keep moving or be slowly dragged in.
- **Counter**: Positioning matters. The player can use the gravity anchors at the corners to resist the pull. Standing behind a wall gives partial cover (the pull is reduced by 50% through solid objects).

**Attack Pattern 2 — Gravity Well** (every 4s):
- The Singularity fires a "gravity well" projectile at a random position in the room. The well is a stationary sphere that creates a local pull of 150 px/s² in a 200px radius for 4s.
- If the player is pulled into the well, they take 1 damage and are held for 0.5s.
- Wells can overlap — if two wells are active simultaneously, their pull stacks.
- **Tell**: The Singularity's lensing ring brightens, then a streak of light shoots to the target position. The streak is visible for 0.5s before the well forms.
- **Counter**: Don't stand still. The wells are predictable — they land at positions the player was at 0.5s ago. Kite them to the same spot to stack them (the overlapping zone is dangerous but predictable). The Rocket Launcher can destroy a gravity well (2 hits).

**Attack Pattern 3 — Event Horizon** (Phase 2, ≤50% HP):
- The Singularity's pull triples. The gravity anchors stop working. The player must stay in motion constantly — stopping for 1s means reaching the center and taking 3 damage (instant kill for builds below 4 HP).
- The room's walls begin to "collapse" inward (visual: walls move 50px closer every 5s, shrinking the arena).
- Small "void spikes" erupt from the floor at random intervals — these deal 0.5 damage and slow the player for 0.5s.
- **Counter**: This is a panic phase — the player has ~40 seconds to kill the Singularity before the room shrinks to unmanageable size. High-DPS weapons (Sniper, Beam with pierce) are ideal. Mobility items (Featherlight, speed shrines) are survival-critical. The player should clear the room of other enemies BEFORE passing the 50% HP threshold — adds become impossible to dodge in the shrinking arena.

**Phase 2 transition**: At 50% HP, the Singularity's lensing ring explodes outward in a visible shockwave. The explosion doesn't deal damage but KNOCKS the player back to a wall. The room's gravity anchors short out (spark effect). The Singularity grows 25% larger. The void spikes begin. There's a 2s pause after the shockwave before the Event Horizon activates — the player should use this to reposition to a corner.

---

### Boss-Existing-System Interaction Summary

| Boss | Cascade | Curse | Rust | Weapon Swap | Extraction Memory |
|------|---------|-------|------|-------------|-------------------|
| Cinder-King | Phase 2 uses Cascade Echoes | Higher curse = more skeletal hands | Rust level increases fire trail duration | Swap to clean weapon to handle hands | (future: boss memory of past runs) |
| The Hail | Cascade disables merge | Higher curse = more enemies absorbed | Rust level increases slide on floor | Shotgun pushes mini-Hails | (future: run history affects boss) |
| The Furnace | Double spawn rate during Cascade | Higher curse = more chain sweeps | Rust level increases vent damage | Beam hits both elemental + Furnace | (future: persists across runs) |
| The Bloom | Overdrive exposes core | Higher curse = more pollen clouds | Rust level speeds root lines | Use beam/pierce through clouds | (future: remembers killed variants) |
| The Singularity | (passive) pull unaffected | Higher curse = stronger pull | Rust level increases pull directly | Mobility weapons help survive phase 2 | (future: boss evolves per run) |

---

## Part D: Content Roadmap — From "Good Roguelike" to "Must-Play"

### The Honest Assessment

Rustworn has ~12,300 lines of Rust, 8 weapons, 19 enemies, 5 bosses, 12 items, 12 synergies, 4 characters, 25 unlocks, 5 biomes. **This is a content-complete indie game prototype.** It's more finished than many Early Access launches.

But content volume isn't the gap. The gap is **identity**. The game needs one thing that makes a player tell a friend about it. Not "it has lots of guns" (every roguelike does). Not "it has 5 biomes" (minimum viable). One *moment*.

### #1: The Hook (Ship This First)

**Feature**: Rust Accumulation + Cascade Echoes as the game's identity.

**What it takes to implement**:

| Component | Effort | Priority |
|-----------|--------|----------|
| RustLevel resource + room-clear increment | ~20 lines, 1 hour | P0 |
| RustedRoom component + slick physics | ~80 lines, 2 hours | P0 |
| Corrosion clouds from rusted walls | ~60 lines (reuse projectile system), 1 hour | P0 |
| WeaponRust component + damage penalty | ~50 lines, 1 hour | P0 |
| Rust visual tint on walls/enemies | ~30 lines (lerp from biome_accent), 30 minutes | P0 |
| Cascade Echo zones on room entry | ~100 lines, 2 hours | P1 |
| Curse ↔ rust level hook for loot calc | ~15 lines, 30 minutes | P1 |
| Remove ×1.08/×1.05 HP scaling | ~30 lines removal, 15 minutes | P0 |
| Cascade Echo visual (time-frozen ghosts in zones) | ~80 lines, 2 hours | P2 |

**Total**: ~6-8 focused hours. **Ship this first.** Without it, the game has no soul.

### #2: Boss Rework (The Why-You-Keep-Playing)

**Feature**: Replace biome bosses with the new designs above.

**Effort**: ~3-4 hours per boss (including tuning, testing, visual placeholder work).

Priority order:
1. **The Bloom** (Verdant) — interacts with Cascade most directly, easiest to implement with existing pollen/root systems
2. **The Furnace** (Magma) — stationary boss is the simplest new architecture, no movement AI needed
3. **The Hail** (Frost) — split/merge mechanic is the most technically novel, needs several new components
4. **Cinder-King** (Catacombs) — fire trail + skeletal hands are medium complexity
5. **The Singularity** (Void) — gravity physics is the most technically challenging, needs new system for pull force

**Total**: ~15-20 hours for all 5.

### #3: Minimum Content to Go from "Good" to "Must-Play"

This is a checklist of things that don't exist yet but are table stakes for a Steam launch:

#### Must-Have (Blocking Steam Release)

| Item | Status | Effort |
|------|--------|--------|
| Rust Identity Hook | Not started | ~6h |
| Boss Rework (5 bosses) | Not started | ~20h |
| Tutorial / First-Run Experience | Not started | ~8h |
| Soundtrack (adaptive, per biome) | 21 procedural .ogg sounds exist | ~15h for full OST |
| Menu Art / Title Screen | Not started (title text only) | ~5h |
| Controller Support (full) | Partial (gamepad input stub) | ~4h |
| Achievements (10-15) | Not started | ~3h |
| Options Menu (rebindable keys) | Settings menu exists (volume + fullscreen) | ~2h to add rebinds |
| Save/Load Meta-Progression | Not started (resets on restart) | ~4h |
| Loading Screen | Not started (instant start = no loading yet) | ~1h |

#### Should-Have (Strongly Recommended for Launch)

| Item | Status | Effort |
|------|--------|--------|
| Boss Health Bar polish | Exists (basic bar) | ~2h |
| Screen Shake / Hitstop VFX | Exists (hitstop) | ~3h to polish |
| Damage Numbers | Exists | ~1h to polish |
| Weapon Rarity System (PLAN.md §5) | Not started | ~8h |
| 2-3 more items + synergies | 12/12 exist | ~3h |
| Steam Integration (achievements, cloud saves) | Not started | ~8h |
| Localization (English + 2 more languages) | Not started | ~10h |

#### Nice-to-Have (Post-Launch or Stretch)

| Item | What it Adds |
|------|-------------|
| 2 more playable characters | 4 exists, 6 is more compelling |
| Daily Seed + Leaderboard | Community engagement |
| Gravestone System (async multiplayer) | Community retention |
| New biome (The Clockwork — gear-themed) | Content expansion |
| Boss Rush Mode | Replayability |

### #4: Steam Store-Ready Checklist

#### Store Page Assets

| Asset | Status | Notes |
|-------|--------|-------|
| **Capsule Art** | Not started | 460x215, 374x448, 616x353. Needs artist. |
| **Screenshots (6-8)** | Not started | Need Rust-themed action shots |
| **GIF / Trailer (30-60s)** | Not started | Show Cascade + Rust + boss fight |
| **Description** | Not started | Lead with Rust identity hook |
| **Tags** | Not started | Roguelike, Action Roguelike, Twin-Stick Shooter, Bullet Hell, Atmospheric |
| **Price Point** | Undecided | Suggest $9.99 (competes with Vampire Survivors price tier) |

#### Technical Requirements

| Item | Status |
|------|--------|
| Windows build | Not tested (Linux-only dev) |
| macOS build | Not tested |
| Linux build | Needs testing on Steam Deck |
| Minimum specs documented | Not started |
| Controller support (full parity) | Partial |
| 60 FPS at 1080p on Steam Deck | Unknown |

#### Launch Checklist

1. **Demo (1-2 months before launch)**: First 2 biomes + 3 bosses + basic meta-progression. Let the Rust hook sell itself.
2. **Beta (2 weeks before launch)**: Full game, limited players, Discord feedback.
3. **Launch day**: All must-haves above, store page live 2 weeks prior.
4. **Post-launch (month 1)**: Bug fix patch. Player feedback features. Performance optimization.
5. **Post-launch (month 2-3)**: New biome (The Clockwork). 2 new characters. Daily seed + leaderboard.

---

## Summary: The One-Page Strategy

```
                    ┌─────────────────────────────────────┐
                    │   PROBLEM: No identity. Adequate    │
                    │   but forgettable roguelike.        │
                    └──────────┬──────────────────────────┘
                               │
                  ┌────────────▼────────────┐
                  │   THE HOOK: Decay +     │
                  │   Time-Scarring          │
                  │   (Rust + Cascade Echoes)│
                  └────────────┬────────────┘
                               │
          ┌────────────────────┼────────────────────┐
          │                    │                    │
          ▼                    ▼                    ▼
   ┌──────────────┐   ┌──────────────┐   ┌──────────────┐
   │Boss Rework   │   │Content Polish│   │Steam Launch  │
   │(5 bosses,    │   │(tutorial,    │   │(capsule,     │
   │system-       │   │controller,   │   │trailer, tags,│
   │interactive)  │   │meta-save,    │   │demo, beta)   │
   │              │   │achievements) │   │              │
   └──────────────┘   └──────────────┘   └──────────────┘
          │                    │                    │
          └────────────────────┼────────────────────┘
                               │
                    ┌──────────▼──────────┐
                    │   MUST-PLAY          │
                    │   ROGUELIKE           │
                    │   "The one where     │
                    │   time leaves scars  │
                    │   and the world rusts│
                    │   around you"        │
                    └─────────────────────┘
```

**The game doesn't need more content. It needs one soul-defining mechanic.** Rust Accumulation + Cascade Echoes are that mechanic, and they're already 70% built on existing systems. The bosses need to interact with that mechanic instead of being HP sponges. Everything else is polish and store-readiness.

**Estimated time to must-play: 40-50 focused hours** (6h hook + 20h bosses + 15h polish/Steam prep + contingency).