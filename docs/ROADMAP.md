# Rustworn — Post-Session Assessment & Roadmap

## Current State (25 commits, ~9,000+ lines, 0 errors)

### ✅ Solid
- Extraction + death screen loop
- 5 biomes + room generation + transitions
- 7 weapons (Pistol, Shotgun, Burst, Beam, Rocket, Sniper, SMG, Arc Gun)
- 4 playable characters with unique abilities
- 25 unlockables (meta-progression)
- Boss health bar + 5 bosses with phase 2 attacks
- Status effects (Burn, Slow, Stun)
- Horde density scaling per floor
- Melee combat system
- 12 in-run items + 3 weapon types
- Hub world with 5 interactive zones + overlays
- Combo system + damage numbers
- Pixel art sprites (14 assets)
- Audio infrastructure (8 event types)
- In-run shop rooms + chests
- Arc Gun chain lightning

### 🟡 Polished but needs work
- Audio: infrastructure exists but no actual .ogg sound files
- Pixel art: sprites exist but need refinement (colored-entity fallback for 4 enemy types)
- Controller support: not wired
- Settings menu: not implemented
- Tutorial/onboarding: missing

### ❌ Missing for marketability

#### Tier 1 — Player-facing (biggest impact)
1. **Real audio** — placeholder-free sounds for weapons, hits, pickups, boss roar, death/win
2. **Settings menu** — volume sliders, resolution, key rebinding
3. **More enemy variety** — 2-3 biome-specific enemy types per biome (currently 4 generic types)
4. **Title screen polish** — animated background, particle effects, game title
5. **Weapon/item synergies** — Isaac-style combos between items

#### Tier 2 — Depth
6. **Daily challenge mode** — fixed seed, leaderboard potential
7. **Achievements** — Steam integration prep
8. **Controller support** — gamepad inputs
9. **More room types** — challenge rooms, sacrifice rooms, secret rooms
10. **Item synergy system** — specific 2-item combos unlock bonus effects

#### Tier 3 — Content
11. **More bosses per biome** — 2-3 per biome instead of 1
12. **Enemy scaling** — enemy HP/damage scales with floor + curse
13. **Visual novel / lore** — NPC dialogue, backstory
14. **Steam integration** — achievements, cloud saves

## What I'd do next (highest impact per effort)

| Priority | Feature | Effort | Impact |
|----------|---------|--------|--------|
| **1** | **Audio** — generate/buy placeholder .ogg files | Medium | **Huge** — silent game feels broken |
| **2** | **Settings menu** — volume + key rebinds | Medium | **High** — basic UX requirement |
| **3** | **Biome-specific enemies** — 2-3 per biome | High | **High** — variety = replayability |
| **4** | **Synergies** — item interactions | High | **Very high** — the game-defining feature |
| **5** | **Daily challenge** — fixed seed runs | Medium | **High** — community engagement |