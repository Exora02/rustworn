//! Pickup, shrine, shop, breakable, HUD, and app-state components —
//! everything that doesn't belong to the player, enemy, or world domains.

use bevy::prelude::*;

// ─── Phase 1 components ───────────────────────────────────────────────────

/// Phase 1 (Vampire Survivors fusion): a pickup dropped by a killed
/// enemy. Drifts gently toward the player each frame (or sits still
/// in the world) and auto-applies its effect on contact. The drop
/// chance is on `GameSettings` per archetype; chasers drop more
/// often than snipers. Effects are permanent for the run (Phase 1
/// is "kill → get stronger", not a slot system — Phase 2 will
/// introduce a choose-1-of-3 chest).
#[derive(Component)]
pub struct Pickup {
    pub kind: PickupKind,
    pub lifetime: Timer,
}

/// Brotato/Gungeon-style run item kind. Each variant modifies an
/// expanded RunState stat when picked up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    /// +0.5 base_damage
    RustyNails,
    /// +5% crit_chance
    LuckyCharm,
    /// +10% move_speed_mult
    Featherlight,
    /// +1 pierce
    SteelToe,
    /// +0.5 hp_regen_per_sec
    Bandage,
    /// +10 pickup_range
    MagnifyingGlass,
    /// +50% damage, -30% max HP
    GlassCannon,
    /// +5% lifesteal_pct
    VampireFangs,
    /// +20 explosion_radius
    ExplosiveCharge,
    /// +1.0 shield
    EnergyShield,
    /// +10% luck
    LuckyHorseshoe,
    /// +20% all stats, +1 curse
    CursedRing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickupKind {
    /// +0.5 HP/sec regen for the rest of the run.
    Regen,
    /// +25% damage on all weapons.
    DamageUp,
    /// +30% fire rate on all weapons.
    FireRateUp,
    /// +20% move speed for the rest of the run.
    SpeedUp,
    /// Run item that modifies expanded stats.
    Item(ItemKind),
    /// Weapon pickup. Adds the weapon to the first empty
    /// inventory slot. If the inventory is full, the pickup
    /// is silently destroyed (no swap UI in P4).
    ///
    /// Phase 2.4: P4 loot drops. The weapon is a fresh copy
    /// of the archetype's base stats (no rarity yet — that's
    /// a future PLAN.md §5 mod-system patch). The pickup
    /// drops per-archetype chance on `GameSettings`
    /// (`pickup_drop_chance_weapon_*`).
    Weapon(crate::components::player::WeaponArchetype),
    /// Heal orb: +3 HP on pickup (one-shot). Dropped by
    /// breakables and as a secondary roll on enemy kills.
    Heal,
    /// Phase 3c gold coin: adds N gold to RunState on pickup.
    /// Dropped by enemies and breakables; spent in shop rooms.
    Gold(u32),
}

/// Multi-slot weapon inventory. The player has 4 slots (PLAN.md
/// §5) — 2 to 6 is the meta-progression range, but 4 is the
/// Phase 2 default. `None` is an empty slot; `Some(archetype)`
/// is a weapon the player owns.
///
/// The player entity ALSO has a `Weapon` component that holds
/// the ACTIVE weapon's full state (damage, fire_timer, burst
/// state, etc.). The inventory is the source of truth for
/// "what the player owns"; the `Weapon` component is the
/// source of truth for "what the player is currently using".
/// Switching weapons (via 1-4 hotkeys) copies the inventory
/// slot's archetype + its base stats into the `Weapon` component.
///
/// P4 scope: picking up a weapon adds it to the first empty
/// slot. If the inventory is full, the pickup is silently
/// destroyed (no swap UI yet — that's a future patch that
/// needs a confirmation prompt). The active weapon is
/// unchanged on pickup.
#[derive(Component, Clone, Debug)]
pub struct WeaponInventory {
    pub slots: [Option<crate::components::player::WeaponArchetype>; 4],
    pub active_slot: usize,
}

impl Default for WeaponInventory {
    fn default() -> Self {
        Self {
            slots: [Some(crate::components::player::WeaponArchetype::Pistol), None, None, None],
            active_slot: 0,
        }
    }
}

/// Tracks per-slot auto-attack cooldowns. Each slot index (0..3) has
/// an independent timer. Removed entirely when the player has < 2
/// weapons (auto-attack disabled).
#[derive(Component, Clone)]
pub struct AutoAttackCooldowns {
    pub cooldowns: [Timer; 4],
}

impl Default for AutoAttackCooldowns {
    fn default() -> Self {
        Self {
            // All timers start finished (ready to fire immediately).
            cooldowns: core::array::from_fn(|_| Timer::from_seconds(0.0, TimerMode::Once)),
        }
    }
}

/// The enemy's "true" color — its archetype's base tint. Stored as a
/// component (rather than derived from `EnemyAI::behavior`) so the
/// hit-flash system can reset/lerp from it without re-running the
/// archetype match, and so future visual effects (status tints,
/// cascade tint) can read it the same way. Set at spawn time in
/// `spawn_chaser` / `spawn_dasher`; never changes for the lifetime
/// of the entity.
#[derive(Component, Clone, Copy, Debug)]
pub struct BaseColor(pub Color);

/// Marker for a HUD element. The HUD plugin positions each element
/// at a fixed offset from the player each frame (so the HUD stays
/// screen-stable as the camera follows) and updates its visual to
/// reflect current state (HP ratio, fire-rate progress, kills, time).
#[derive(Component)]
pub struct HudElement {
    pub kind: HudElementKind,
}

#[derive(Clone, Copy, Debug)]
pub enum HudElementKind {
    HpBar,
    FireRate,
    KillBar,
    TimeBar,
    GameOver,
    RestartPrompt,
    /// Phase 1 cascade: a bar that shows the next Cascade window
    /// — filling toward 1.0 while the cooldown counts down, then
    /// draining back toward 0.0 while the window is active. Hidden
    /// entirely when no cooldown is in flight (so the HUD is quiet
    /// outside the cycle). Positioned just below the time bar.
    CascadeBar,
    /// Phase 1 mini-boss: a prominent HP bar at top-center, 60%
    /// of the viewport wide, 8px tall, deep red on a dark
    /// background. Hidden by default; the HUD update system flips
    /// it to `Visibility::Inherited` only when a `With<Boss>`
    /// entity exists, and back to `Visibility::Hidden` when
    /// the boss is killed. Fill scale = boss's
    /// `Health.current / Health.max`. Lives alongside the
    /// other bars in the HUD; doesn't break their layout.
    BossHpBar,
    /// Phase 2: title-screen overlay. A dim full-screen background
    /// + "PRESS ENTER TO START" text. Hidden by default; the
    /// state_visibility_system flips it to `Visibility::Inherited`
    /// only while `AppState == MainMenu`. Spawned once at startup
    /// (alongside the camera) so the entity is always present and
    /// we just toggle its visibility per state.
    MainMenu,
    /// Variant of GameOver for extraction runs, so the HUD can
    /// render "EXTRACTED!" instead of "GAME OVER".
    GameOverExtracted,
    /// Phase 2 P5: XP counter bar, positioned above the kill bar.
    /// Shows the player's XP-to-10 progress as a thin bar. Color
    /// is a warm gold so it reads as "Cascade progress" rather
    /// than "kill counter" (which is a dim white).
    XpBar,
    /// Phase 3 meta-progression: upgrade shop. Visible only on the
    /// MainMenu screen. Shows Crown balance + 8 upgrade rows.
    MetaShop,
    /// Phase 3: character select overlay on MainMenu.
    CharacterSelect,
    /// Phase 3: pause overlay. Visible only on AppState::Paused.
    PauseOverlay,
    /// Phase 4: floor/biome indicator at top-center.
    /// Shows "Floor N — BIOME NAME" text. Visible only during InRun.
    FloorIndicator,
    /// Phase 4: XP score text next to the XP bar.
    /// Shows "Score: N" in small gold text.
    ScoreText,
}

/// Phase 1 cascade telegraph: a full-screen overlay sprite that
/// tints the world during a Cascade window. Solid purple tint
/// while the window is open, pulsing purple in the 3s before
/// opening, hidden otherwise. Per-entity marker so the
/// cascade-telegraph update system can find the overlay via
/// `Query<&mut Sprite, With<CascadeTelegraph>>` and update its
/// color/alpha/visibility without disturbing any other HUD
/// element. The overlay is positioned at the player each
/// frame so it always covers the camera viewport (the overlay
/// is 2× the view size to ensure it covers even during
/// extreme camera-follow lag).
#[derive(Component)]
pub struct CascadeTelegraph;

/// Phase 1 beam: marker for the visual sprite entity that
/// represents the active beam. A single persistent entity
/// spawned once at startup; `beam_damage_system` updates its
/// position, rotation, scale, and visibility each frame
/// based on the player's position, aim direction, and the
/// `Weapon::beam_active` flag. Lives separately from the
/// player so the beam's z-order, size, and color are
/// independent of the player's transform. The sprite is
/// anchored at `Anchor::CenterLeft` (its transform position
/// is the player's position, the beam extends outward in
/// the +X direction after rotation), so a 0° rotation
/// points the beam along the +X axis and a 90° rotation
/// points it along +Y.
#[derive(Component)]
pub struct BeamVisual;

// ─── Game Over Reason ─────────────────────────────────────────────────────

/// Why the run ended. Used by the HUD overlay to show different
/// end-of-run text ("GAME OVER" vs "EXTRACTED!").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GameOverReason {
    #[default]
    /// Player's HP hit 0.
    Died,
    /// Player voluntarily used the extraction portal.
    Extracted,
}

// ─── AppState (Phase 2) ───────────────────────────────────────────────────

/// Top-level game state. Lifts the Phase 1 `RunState.game_over: bool`
/// into a proper Bevy `States` enum so each major UI/mode gets its
/// own system schedule.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    /// Title screen. Player + spawners exist but are hidden / not
    /// ticking; gameplay systems are gated off. ENTER transitions
    /// to InRun (which fires the per-run reset in
    /// `enter_in_run_system`).
    #[default]
    MainMenu,
    /// Active run. All gameplay systems tick, the HUD shows run
    /// progress, and `player_death_check_system` watches for HP <= 0
    /// to transition to GameOver.
    InRun,
    /// Player died. Gameplay systems are gated off (the world is
    /// frozen on a still frame), the game-over overlay is visible,
    /// and R restarts (→ InRun) or ESC returns to the title (→
    /// MainMenu). The transition InRun → GameOver fires
    /// `enter_game_over_system`, which can hook fade-in effects
    /// in a later patch.
    GameOver,
    /// Paused. Scaffolded for a future pause feature (ESC during
    /// InRun, ESC again to resume). No systems gate on it yet —
    /// it's here so the enum is a complete top-level state model
    /// and we can add the plumbing without another refactor.
    Paused,
}

// ─── Room interactables (Phase 2 P6.2) ─────────────────────────────────────

/// Focus category for a shrine. Determines the pool of random
/// upgrades it can grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShrineFocus {
    /// Heal/regen shrine. Collected INSTANTLY on touch (no
    /// channeling). Rewards: HP, regen, max HP.
    HealRegen,
    /// Combat shrine. Must stand near it for 1.5s to channel.
    /// Rewards: damage, fire rate, projectile speed.
    Combat,
    /// Utility shrine. Must stand near for 1.5s to channel.
    /// Rewards: move speed, XP, pickup radius, dash.
    Utility,
    /// Cursed shrine. Visually distinct (dark purple), optional.
    /// Gives curse + big guaranteed reward. Player's choice
    /// to channel or leave it alone.
    Cursed,
}

/// A shrine entity in the world. The player can interact with
/// it for a one-time reward.
///
/// Heal/Regen shrines are instant-collect: touching them applies
/// the reward and despawns the shrine immediately.
///
/// Combat/Utility shrines require the player to stand within
/// range for `channel_duration_s` seconds (1.5). During this
/// time the player can be attacked (risk/reward). If the player
/// leaves the range, the channel resets. On successful channel,
/// a random upgrade from the focus's pool is applied.
#[derive(Component)]
pub struct Shrine {
    pub focus: ShrineFocus,
    /// How long the player MUST stand in range for the shrine
    /// to activate. Heal/Regen shrines set this to 0 (instant).
    pub channel_duration_s: f32,
    /// Current channel progress. Resets to 0 when the player
    /// leaves range.
    pub channel_progress: f32,
    /// True once the reward has been given. The shrine despawns
    /// after activation.
    pub activated: bool,
}

/// Phase 3c: a shop item in a shop room. The player walks over
/// it to buy it. If they have enough gold, the cost is deducted
/// and the reward is applied. The item despawns after purchase.
#[derive(Component)]
pub struct ShopItem {
    pub cost: u32,
    pub reward: ShopReward,
}

/// Rewards a player can buy in a shop room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShopReward {
    DamageUp,
    FireRateUp,
    SpeedUp,
    MaxHpUp,
    Regen,
    HealFull,
    /// Unlocks a random weapon in an empty inventory slot.
    Weapon(crate::components::player::WeaponArchetype),
}

/// A fragile breakable object (pot, crate, barrel). Takes
/// `hp` hits to destroy. On destruction, rolls a loot drop
/// (hearts, gold, pickups). Breakables are hit by player
/// projectiles and beam.
#[derive(Component)]
pub struct Breakable {
    pub hp: u32,
}

/// A Marauder grenade in flight. Travels in a direction, then
/// explodes on enemy contact or after `timer` runs out.
/// Explosion damages all enemies within `range`.
#[derive(Component)]
pub struct Grenade {
    pub timer: Timer,
    pub range: f32,
    pub damage: f32,
    pub direction: Vec2,
    pub speed: f32,
}