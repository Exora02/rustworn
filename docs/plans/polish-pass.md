# Polish & Cleanup Pass

## Code Cleanup

### 1. Apply cargo fix
- `cargo fix --lib -p rustworn --allow-dirty` — fixes deprecated API calls, unused variables, etc.
- Manual fix remaining ~90 warnings

### 2. Dead code audit (biggest chunk — ~50 warnings)
Remove or #[allow] these:
- `src/plugins/rooms/generate.rs` — orphaned module? All constants + functions marked dead
- `src/plugins/enemy/constants.rs` — multiple constants unused
- `src/resources/effects.rs` — BossRampTimer, start_interval unused
- Unused WeaponArchetype variants in shops/rolls
- `BossPhase::Summoning/Charging/Teleporting` — added but never constructed

### 3. Deprecated API fixes (8 warnings)
- `Query::get_single()` → `Query::single()` in boss_attacks.rs (5 calls) + fx.rs (1 call)

### 4. Unused imports (3 warnings)
- Remove unused imports: MetaUpgradeKind, UnlockableDef, spawn_death_particles

### 5. Orphaned files
- `src/plugins/rooms/generate.rs` — check if actually used or if it's a duplicate of mod.rs
- `src/components/player.rs:WeaponArchetype::Beam` — no Beam weapon variant? Actually Beam exists
- Check for duplicate pub constants (e.g., WALL_COLOR, PAD_COLOR, FLOOR_COLORS in both mod.rs AND generate.rs)

## Visual Polish

### 1. Use the pixel art sprites
The assets/art/*.png sprites exist but aren't loaded by the game. Wire them in:
- player.png → Player entity
- enemy_chaser.png / enemy_dasher.png → enemy entities
- weapon_*.png → weapon entities / projectiles
- boss_*.png → boss entities
- ui_heart.png → HUD heart icon

### 2. Add sprite tinting per biome
Each biome should tint sprites slightly (multiply color) — reuse biome_accent().

### 3. HUD visual polish
- Better font rendering (use a pixel font?)
- Health bar styling (heart icon next to HP numbers)
- Floor indicator styling

## What to delegate vs do directly
- Code cleanup (cargo fix + dead code): delegate to glm 5.2
- Sprite wiring (use the art assets): delegate to glm 5.2 (or DS V4 Flash)
- HUD polish: delegate to DS V4 Flash