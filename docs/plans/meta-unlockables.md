# Meta-Progression Rework: Unlockables (Gungeon-style)

**Current:** 8 stat upgrades (MaxHp, StartingGold, ExtraSlot, MoveSpeed, DashCooldown, CascadeDuration, Damage, Regen) — bought with Crowns on MainMenu.

**Target:** Crowns unlock NEW items/weapons that can then appear during runs. The 8 stat upgrades become Crown purchases, but the focus shifts to permanent unlockables.

## Design

### Unlockable Table
| Index | Name | Cost | Effect | Category |
|-------|------|------|--------|----------|
| 0 | **Beam** | 10 Cr | Unlock Beam weapon (appears in pickups) | Weapon |
| 1 | **Burst** | 10 Cr | Unlock Burst weapon | Weapon |
| 2 | **Rocket Launcher** | 15 Cr | Unlock Rocket Launcher | Weapon |
| 3 | **Crown of Greed** | 20 Cr | +50% gold from all sources | Passive |
| 4 | **Glass Cannon** | 25 Cr | +2 damage, max HP capped at 2 | Passive |
| 5 | **Explosive Rounds** | 20 Cr | Projectiles have 20px AOE | Passive |
| 6 | **Thorns** | 15 Cr | Enemies take 1 damage on hitting you | Passive |
| 7 | **Vampire** | 30 Cr | Heal 0.5 HP on kill | Passive |

### Implementation

### 1. Refactor MetaConfig + CrownSave
Replace the 8 `MetaUpgradeKind` stat upgrades with `UnlockableKind`.

In `src/resources/meta.rs`:
```rust
pub enum UnlockableKind {
    Beam,
    Burst,
    RocketLauncher,
    CrownOfGreed,
    GlassCannon,
    ExplosiveRounds,
    Thorns,
    Vampire,
}
```

Each unlockable has a `name`, `base_cost`, and `max_level` of 1 (it's binary — locked/unlocked).

### 2. Unlockable items appear in runs
When a run starts, check which unlockables are unlocked:
- Beam/Burst/RocketLauncher: add to the weapon pickup pool
- CrownOfGreed: apply `run.gold_mult = 1.5` in `enter_in_run_system`
- GlassCannon: apply `run.damage_stacks += 2` + `h.max = 2` in `enter_in_run_system`
- ExplosiveRounds: add to projectile system (maybe skip for now, too complex)
- Thorns: system that damages enemies hitting player (skip for now)
- Vampire: system that heals on enemy kill

### 3. Keep existing Crown save format
- `save.crowns` stays
- `save.unlocks: [bool; 8]` instead of `save.upgrades: [u32; 8]`
- Migration: current `save.upgrades[i] > 0` → `save.unlocks[i] = true`

### 4. Shop UI changes
- Show "[UNLOCKED]" vs "LOCKED (10 Cr)" instead of levels
- Keys 1-8 buy the unlockable
- Show a star/crown indicator next to each

### 5. Weapon pool integration
In `src/plugins/pickups.rs` or `src/plugins/combat/mod.rs`:
- When spawning weapons in rooms, filter by unlocked set
- If only Pistol is unlocked, only Pistol appears
- Unlock more → weapon variety increases in runs

## Files to modify
- `src/resources/meta.rs` — UnlockableKind enum, new MetaConfig
- `src/resources/mod.rs` — re-export
- `src/plugins/setup.rs` — enter_in_run_system, menu_input_system, shop text
- `src/plugins/pickups.rs` — weapon pool filtering
- `src/components/misc.rs` — remove unused/unlockable mapping (if needed)