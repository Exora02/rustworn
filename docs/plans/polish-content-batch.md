# Polish + Content Batch

## Game Feel (DS V4 Flash)

### 1. Hit flash on enemies
When an enemy takes damage, flash them white for 0.1s.
- Already partially exists: HitFlash component in src/components/enemy.rs
- Just ensure it's applied on all damage paths (projectile, beam, melee)
- In projectile.rs and damage.rs: add `commands.entity(enemy).insert(HitFlash { timer: Timer::from_seconds(0.1, TimerMode::Once) })` when damage is dealt

### 2. Floating damage numbers (Brotato-style)
When an enemy takes damage, spawn a small text entity at the hit position that floats up and fades:
- Text shows the damage amount (integer)
- Starts at enemy_pos, floats upward at 30px/s
- Fades alpha from 1.0 to 0.0 over 0.6s
- Color: white, font_size: 14.0
- Has a DamageNumber component with lifetime Timer
- System ticks them: move up, fade alpha, despawn on expiry
- Add DamageNumber to src/components/enemy.rs if not there

### 3. Weapon swap UI overlay
Show current weapon inventory as a HUD overlay:
- When player presses Tab or the weapon swap key, show a translucent overlay
- Overlay shows: "1: Pistol [ACTIVE]\n2: Shotgun\n3: [empty]\n4: [empty]"
- Position: bottom-center, above the floor indicator
- Use existing HUD text pattern from bars.rs
- Auto-hide after 1.5s of no weapon switching

### 4. Combo/score system
Track a combo counter that increases with quick kills:
- Add to RunState: `combo: u32 = 0`, `combo_timer: Timer` (3s)
- On enemy kill: increment combo, reset timer to 3s
- If timer expires: reset combo to 0
- Higher combo = more score on death screen
- Display combo as a small HUD text: "x5 COMBO"
- Color shifts from white → yellow → red as combo increases

## Content (glm 5.2)

### 1. Arc Gun weapon wiring
ArcGun is in WeaponArchetype enum but has no behavior. Wire it:
- Arc gun fires a projectile that chains between enemies
- On hit: find nearest enemy within 100px, chain a small lightning bolt (reuse beam visual or spawn a small colored line)
- Chain up to 3 targets
- Damage: 0.8 per hit, fire_rate: 2.0, speed: 400
- Visual: yellow projectile sprite (use weapon_beam.png)

### 2. Shop rooms in biomes
Currently shops only exist in the hub. Add in-run shops:
- When entering a shop room (room_type == Shop), show a shop UI overlay
- 3-4 items for sale with gold prices
- Items are from the expanded stats pool (RunState modifiers)
- Player presses 1-4 to buy, ESC to leave
- Prices: 5-15 gold per item

### 3. Chests in rooms
Replace some breakables with chests:
- Chest component: locked/unlocked, contents (PickupKind)
- 1-2 chests per room, 2-3 in elite rooms
- Opening: walk over and press E (or auto-open on contact)
- Drop a random item: gold, weapon, heal, or run item
- Visual: yellow/gold colored rectangle (larger than breakables)

### 4. Pickup juice
- When a pickup is collected, briefly scale it up 1.5x and flash
- Small particle burst on collect (4 small colored circles)
- Use existing spawn_death_particles but with pickup color instead