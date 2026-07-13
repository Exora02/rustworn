# Rustworn — Session Roadmap (Updated)

## Current State (28 commits this session, ~12,300+ lines, 0 errors)

### ✅ Delivered This Session
- **Audio** — 21 procedural .ogg sounds wired into all game events (weapons, hits, pickups, dash, cascade, rooms, UI)
- **Settings menu** — volume sliders (master/SFX/music), fullscreen toggle, ESC from hub
- **Title screen polish** — floating ember particles, pulsing title text, portal sparkles
- **15 biome-specific enemies** — 3 per biome with unique attack patterns (skeleton archers, ghosts, ice golems, fire mages, void walkers, etc.)
- **Item synergies** — 12 pair combos (SuperPierce, EchoChain, ExplosiveRetaliation, DoubleEdge, etc.)
- **More room types** — Treasure rooms (pick 1 of 3 items), Challenge rooms (3 locked waves), Secret rooms (hidden, free loot)
- **Game feel** — damage numbers, combo system, weapon swap UI, hit flash
- **Arc Gun chain lightning** — chains to 2 nearby enemies
- **Chests in rooms** — breakable replacements with loot
- **In-run shop rooms** — 4 items for sale with gold

### ✅ Solid (pre-session)
- Hub world + 25 unlockables + meta-progression
- 8 weapons + 4 characters + 5 bosses
- Extraction loop + death screen
- Status effects, horde scaling, melee
- 12 run items + 3 weapon unlocks

### ❌ Still Missing

| Priority | Feature | Effort | Why |
|----------|---------|--------|-----|
| **1** | **Code audit + refactor** | Medium | Warnings are at 114, code quality is the enabler for everything else |
| **2** | **Daily challenge mode** | Medium | Fixed seed runs, leaderboard potential |
| **3** | **Controller support** | Medium | Gamepad inputs for Steam/console |
| **4** | **More bosses per biome** | High | 2-3 per biome instead of 1 |
| **5** | **Enemy scaling (floor+curse)** | Low | HP/damage scaling for depth |
| **6** | **Tutorial / onboarding** | High | First-run flow |
| **7** | **Lore / NPC dialogue** | High | World-building |
| **8** | **Achievements (Steam)** | Medium | Steam integration |