# Hub World + Expanded Unlockables Plan

## Hub World (Gungeon-style)

### Design
Replace the current MainMenu/CharacterSelect text menu with a visual hub screen:
- A room background with visual elements
- Player character walks around
- 5 interactable NPCs/objects:
  1. **Run Portal** (center) — "Enter the Depths" → starts a run
  2. **Shopkeep** (left) — buy items for Crowns
  3. **Unlockable Wall** (right) — shows locked/unlocked items
  4. **Class Select** (top-left) — change character
  5. **Stats Board** (top-right) — show run history/stats

### Implementation
Rather than building a complex 2D room with NPC sprites, start with:
- A nicer main menu background (tiled floor pattern)
- 5 interactable zones highlighted with colored rectangles
- Mouse click / keyboard navigation between zones
- Keep the existing text overlays but reorganize them into zones

### Files
- `src/plugins/hub/mod.rs` — new hub plugin
- `src/plugins/hub/render.rs` — hub background rendering
- `src/plugins/hub/interaction.rs` — click/keyboard interaction
- Modify `setup.rs` — replace spawn_world for MainMenu with hub

## Expanded Unlockables (20-30)

We currently have 8 unlockables. Need to expand to ~25.

### New unlockable categories:

**Weapons (3 new = 6 total unlockable weapons):**
- Beam, Burst, RocketLauncher (existing)
- **Sniper** (15 Cr) — long range, high damage, slow fire
- **SMG** (12 Cr) — fast fire, low damage, spray pattern
- **Arc Gun** (18 Cr) — chain lightning between nearby enemies

**Passive items (10 new):**
- Sharpened Edge (+1 pierce)
- Rabbit's Foot (+15% luck)  
- Titanium Plating (+0.5 armor cap)
- Adrenaline (+20% speed when HP < 30%)
- Scope (+15% range)
- Heavy Magazine (+20% fire rate, -15% move speed)
- Conducting Rod (projectiles chain to 2 nearby enemies)
- Reactive Armor (taking damage releases a shockwave)
- Blood Price (heal 1 HP, +1 curse)
- Echo Chamber (every 3rd shot is doubled)

**Active items (4 new):**
- Smoke Bomb (teleport dodge, 8s CD)
- Rage Mode (2x damage for 3s, 12s CD)
- Healing Aura (heal 2 HP over 3s, 15s CD)
- Time Warp (50% slow for 2s, 10s CD)

### Implementation
- Add new UnlockableKind variants
- Define costs 12-30 Cr each
- All items modify expanded RunState stats  
- Active items need a new system (press Space/Shift to activate)
- Wire them into the drop pool + shop

### Files
- `src/resources/meta.rs` — expand UnlockableKind to ~25 variants
- `src/plugins/setup.rs` — shop UI, enter_in_run effects
- `src/plugins/pickups.rs` — drop items, apply effects
- New: `src/plugins/combat/actives.rs` — active item system