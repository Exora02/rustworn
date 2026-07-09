use bevy::prelude::*;

/// Phase 2.3: per-run seeded RNG. The single source of
/// randomness for the whole run — every `random_spawn_position`,
/// `boss_spawn_angle`, etc. reads from this resource. The seed
/// is set in `enter_in_run_system` from `RunState.seed` (which
/// itself is set from system time at run start).
///
/// `state` is a 64-bit xorshift register. `next_unit()` returns
/// a uniform f32 in [0, 1). The state is `Cell`-free because
/// `ResMut<RunRng>` gives unique access per-system.
///
/// Why a Resource, not `rand::random()`? `rand::random()`
/// uses a thread-local generator seeded from the OS — runs
/// aren't reproducible, the sequence is global, and the
/// phase-2 multiplayer prototype (Phase 5) needs every
/// client's RNG to advance identically. A `ResMut<RunRng>` is
/// the lightest-weight determinism we can get without a full
/// lockstep layer.
#[derive(Resource)]
pub struct RunRng {
    pub state: u64,
}

impl RunRng {
    /// Construct a new `RunRng` from a 64-bit seed. The seed
    /// 0 is rejected because xorshift's all-zero state is a
    /// fixed point (every `next_unit()` returns 0). The
    /// fallback `0xCAFEBABE` is arbitrary — any non-zero
    /// constant works.
    pub fn new(seed: u64) -> Self {
        let state = if seed == 0 { 0xCAFEBABE } else { seed };
        Self { state }
    }

    /// Advance the xorshift register by one step and return
    /// a uniform f32 in [0, 1). Same algorithm as the Phase 1
    /// ad-hoc helper in `random_spawn_position` and
    /// `boss_spawn_system` — the constants (13, 7, 17) are
    /// the standard "best" xorshift64 magic numbers (per
    /// Vigna, "An experimental exploration of Marsaglia's
    /// xorshift generators, scrambled", 2014).
    pub fn next_unit(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        ((x >> 40) as f32) / ((1u32 << 24) as f32)
    }
}

impl Default for RunRng {
    fn default() -> Self {
        // Seed 0 is rejected inside `new`. A real run picks
        // its seed in `enter_in_run_system` (from
        // system time). The default is just a placeholder so
        // the Resource initializer doesn't panic.
        Self::new(0xDEADBEEF_CAFEBABE)
    }
}
