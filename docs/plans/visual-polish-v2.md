# Visual Polish Pass

## 1. Wire pixel art sprites to entities
A `GameSprites` resource exists in `src/plugins/enemy/spawn.rs` with handles for 14 sprites.
The sprites are not applied to actual entities — everything renders as colored rectangles.

### Player
In setup.rs player spawn: set `Sprite { image: sprites.player.clone(), custom_size: Some(Vec2::new(40.0, 40.0)), ..default() }`

### Enemies (chaser, dasher)
In spawn.rs `spawn_chaser` / `spawn_dasher`: set sprite image to `sprites.enemy_chaser` / `sprites.enemy_dasher`. Keep existing biome color tint on `Sprite::color`.

### Bosses
In spawn.rs `spawn_boss`: match Biome → boss sprite (Lich/Yeti/Inferno/Treant/VoidWalker)

### Weapon projectiles
In auto_attack.rs `spawn_projectile()` and projectile.rs: match WeaponArchetype → weapon sprite. Use `custom_size: Some(Vec2::new(12.0, 12.0))` for projectiles.

### Weapon pickups
In pickups.rs `spawn_pickup`: the function already takes `sprites: Option<&GameSprites>`. Apply sprite image for weapon archetype pickups.

## 2. Hub visual polish

### Portal glow
In hub/mod.rs: add a pulsing glow behind the "ENTER THE DEPTHS" zone using `sin(time * 2.0)` to oscillate alpha between 0.4-1.0.

### Zone hover highlight
In hub_interaction_system: check mouse position vs zone transforms. On hover, brighten color by 1.3x.

### Zone borders
Each zone gets a slightly larger (2px) border entity behind it with 1.3x lighter color.

## 3. Room floor grid
In rooms/mod.rs `spawn_walls()`: after spawning the floor, spawn thin 1px grid lines every 40px horizontally and vertically using the biome accent color at 20% opacity.