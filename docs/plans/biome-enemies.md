# Biome-Specific Enemies — 15 New Enemy Types

## Design
Each biome gets 3 unique enemy types with distinct behaviors, sprites, and attack patterns:
The existing 4 generic types (chaser, dasher, shooter, bomber) remain as biome-agnostic filler.
New enemies are biome-specific and appear alongside generics.

### Catacombs (dark/purple necromancer theme)
1. **Skeleton Archer** — Shoots bone arrows. Stands still and fires at player. Medium HP, low speed.
   - AiBehavior::SkeletonArcher variant: stands still, fires a piercing projectile every 1.5s
2. **Ghost** — Phases through walls. Low HP, medium speed. Cannot be hit while phasing.
   - AiBehavior::Ghost: moves toward player, periodically becomes semi-transparent (collision off)
3. **Skull Swarm** — 3 small skulls that orbit a point and rush the player.
   - Spawn 3 small entities that orbit a center point, then lunge at player every 3s

### Frost (blue/white ice theme)
1. **Ice Golem** — Slow, high HP. AOE slam attack.
   - AiBehavior::IceGolem: slow approach, every 3s does a ground slam (spawns expanding ring)
2. **Snow Drifter** — Fast zigzag pattern. Leaves slowing trail.
   - AiBehavior::SnowDrifter: fast zigzag movement, leaves behind slowing zones
3. **Penguin Bomber** — Waddles toward player, explodes on death.
   - AiBehavior::PenguinBomber: quick movement, on death creates a small explosion

### Magma (red/orange fire theme)
1. **Fire Mage** — Teleports around, shoots fireballs.
   - AiBehavior::FireMage: teleports every 2s, shoots 3 fireballs in a spread
2. **Lava Elemental** — Splits into 2 smaller elementals on death.
   - AiBehavior::LavaElemental: medium speed, medium HP. On death, spawns 2 smaller versions
3. **Cinder Wisp** — Flies fast, leaves fire trail.
   - AiBehavior::CinderWisp: fast, leaves damaging fire patches on the floor

### Verdant (green nature theme)
1. **Treant** — Slow, high HP. Roots player in place.
   - AiBehavior::Treant: slow, every 4s shoots roots from ground (temporary slow zone)
2. **Vine Creeper** — Moves along walls/edges, ambushes.
   - AiBehavior::VineCreeper: moves along room edges, dives at player when close
3. **Spore Pod** — Stationary, shoots spore clouds.
   - AiBehavior::SporePod: stationary, shoots poison clouds that linger

### Void (dark purple void theme)
1. **Void Walker** — Teleports behind player, melee attack.
   - AiBehavior::VoidWalker: teleports behind player, does a melee slash every 2s
2. **Corruption Orb** — Floating orb that shoots seeking projectiles.
   - AiBehavior::CorruptionOrb: floats, shoots slow homing projectiles
3. **Shadow Clone** — Copies player's movement mirrored.
   - AiBehavior::ShadowClone: mirrors player position with offset, shoots at player

## Implementation

### New AiBehavior variants
In src/components/enemy.rs, add 15 new AiBehavior variants.

### Enemy spawning
In src/plugins/enemy/spawn.rs, the spawn/biome system:
- When spawning enemies for a biome, pick from the biome's pool instead of only generics
- Keep chance of spawning generics at ~40%, biome enemies at ~60%

### Visual
Use colored rectangles for now (same as generic enemies). Each enemy gets a distinct color + shape:
- SkeletonArcher: dark bone color, rectangle
- Ghost: semi-transparent white
- IceGolem: light blue, large square
- etc.

### New attack patterns
- Arrows/bone projectiles: reuse existing projectile system
- AOE slam: spawn expanding ring collider (temporary)
- Slowing trail: spawn floor zones with Slow status
- Fire trail: spawn damaging floor zones
- Teleport: instant position change
- Split on death: spawn 2 smaller versions
- Roots: spawn temporary slowing zone
- Homing projectiles: slight course correction toward player
- Mirror: read player transform, mirror it

## Files to modify
- src/components/enemy.rs — AiBehavior enum (15 new variants)
- src/components/mod.rs — any new components
- src/plugins/enemy/spawn.rs — spawn function per enemy type
- src/plugins/enemy/mod.rs — register new systems
- src/plugins/enemy/waves.rs — wave spawning that includes biome-specific enemies
- src/plugins/combat/projectile.rs — new projectile patterns (homing, piercing, fireballs)
- src/plugins/combat/status.rs — floor zones (fire, slowing)