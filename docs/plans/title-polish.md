# Title Screen Polish Plan

## Design
The current title screen is the hub with "ENTER THE DEPTHS" as the default. Add more juice:

1. **Animated particles** in the background:
   - Floating embers/embers drifting upward (small orange/yellow circles)
   - Slow, random drift
   - 20-30 particles at a time, continuous spawn
   - Use existing particle patterns from fx.rs

2. **Game title text:**
   - Large "RUSTWORN" text centered above the hub zones
   - Golden/rust-colored
   - Subtle pulsing animation (scale oscillates 1.0-1.05)

3. **Portal visual enhancement:**
   - The ENTER THE DEPTHS zone already has a glow
   - Add small particle sparks shooting out of it
   - More dramatic entrance

## Implementation
In src/plugins/hub/mod.rs:
- Add title_particles_system that spawns/updates particles
- Add title_text entity in spawn_hub_menu
- Add title_animation_system for pulsing