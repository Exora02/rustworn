# P6.3 — Boss rooms + difficulty scaling + determinism

Three changes in one patch:

**1. Boss rooms on entry**
- boss_spawn_system now also fires immediately when entering
  a Boss-kind room (not just on timer)

**2. Difficulty scaling**
- New helper `floor_difficulty_mult(floor) → (hp, dmg)`
  hp = 1.0 + floor * 0.08
  dmg = 1.0 + floor * 0.05
- Applied in spawn_X functions (HP) and combat damage (dmg)

**3. Determinism P7 cleanup**
- combat.rs: replace 2x DefaultHasher rng closures with RunRng
- rooms.rs: replace DefaultHasher in pick_shrine_reward with RunRng