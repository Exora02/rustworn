# Rocket Launcher Weapon Archetype

**Goal:** Add a 5th weapon archetype: Rocket Launcher — slow firing, big explosion AOE.

## Implementation

### 1. Add variant to WeaponArchetype
In `src/components/player.rs`, add:
```rust
/// Phase 4 rocket launcher: slow-firing projectile that explodes
/// on contact for area damage.
RocketLauncher,
```

### 2. Add stats in weapon_stats_for
In `src/plugins/combat/mod.rs`, add:
```rust
WeaponArchetype::RocketLauncher => (3.0, 0.8, 400.0, 1, 0.0, 1),
```
(damage=3, fire_rate=0.8 rps, speed=400, 1 pellet, no spread, 1 burst round)

### 3. Add next_archetype cycle
Cycle order: Pistol → Shotgun → Burst → Beam → RocketLauncher → Pistol

### 4. Rocket projectile behavior
In `src/plugins/combat/projectile.rs`, modify the projectile spawning or add a system:
- When rocket projectile hits an enemy (or reaches max lifetime ~3s), explode
- Explosion: damage all enemies within 80px radius for `damage` amount
- Use the existing _damage_in_area or similar pattern
- Visual: rocket is a larger (20px) red square projectile

Keep it simple — reuse the existing projectile + damage infrastructure.

## Files to modify
- `src/components/player.rs` — add `RocketLauncher` variant
- `src/plugins/combat/mod.rs` — add stats, cycle order
- `src/plugins/combat/projectile.rs` — add explosion event/handler or modify collision

## Build check
`docker exec rustworn-dev bash -l -c "cd /app && cargo check 2>&1 | grep -E '^error'"`