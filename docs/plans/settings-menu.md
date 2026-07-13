# Settings Menu Plan

## Design
Add an in-game settings menu accessible from the hub:
- Press ESC in hub → show settings overlay
- Or add a "SETTINGS" zone to the hub (6th zone)

### Settings
1. **Volume sliders:**
   - Master Volume (0-100%)
   - SFX Volume (0-100%)
   - Music Volume (0-100%) — future use

2. **Key rebinding** (minimal):
   - Show current key bindings as text
   - Display: "Move: WASD | Shoot: Mouse | Dash: Space | Swap: Q/E | Ability: F"

3. **Display:**
   - Fullscreen toggle (Window / Fullscreen)

### Implementation
Create `src/plugins/settings.rs`:
- SettingsResource with volume levels and display mode
- `SettingsPlugin` with overlay system
- ESC to open/close, Arrow keys to change values

Keep it simple — text-based overlay like the shop/unlockables overlays.