fn enemy_ai_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    // `Option<&mut DasherState>` lets the same query match both
    // archetypes — chasers get None, dashers get Some. The new
    // `Option<&mut SniperState>` adds the 4th archetype the
    // same way: chasers/dashers/shooters get None, snipers get
    // Some. `&mut Sprite` lets the sniper's AI lerp the sprite
    // color from yellow to red during the charge (the visual
    // telegraph) — non-sniper entities never have their color
    // touched in this system.
    mut enemies: Query<
        (
            &Transform,
            &EnemyAI,
            &mut BaseVelocity,
            &mut Sprite,
            &Health,
            Option<&mut DasherState>,
            Option<&mut ShooterState>,
            Option<&mut SniperState>,
            Option<&mut BossState>,
            Option<&mut OrbiterState>,
        ),
        With<Enemy>,
    >,
) {
    if hitstop.is_some() {
        return;
    }
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();

    for (
        enemy_transform,
        ai,
        mut base,
        mut sprite,
        health,
        dasher_state,
        shooter_state,
        sniper_state,
        boss_state,
        orbiter_state,
    ) in &mut enemies
    {
        match ai.behavior {
            AiBehavior::Chaser => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.chaser_move_speed;
            }
            // Phase 3d bomber: walks toward the player at
            // bomber_move_speed. Same movement pattern as
            // Chaser but with its own speed tuning.
            AiBehavior::Bomber => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.bomber_move_speed;
            }
            AiBehavior::Dasher => {
                let Some(mut state) = dasher_state else {
                    // Defensive: a dasher without DasherState shouldn't
                    // exist (spawn_dasher always attaches one) but
                    // skipping is safer than panicking.
                    continue;
                };
                // Tick the phase timer. If it just elapsed, advance
                // to the next phase and reset the timer to the new
                // phase's duration. The to_player direction is
                // re-computed each phase (so the dash re-aims if
                // the player moved during Approach).
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                let (speed, next_phase, next_duration) = match state.phase {
                    DasherPhase::Approach => (
                        settings.dasher_approach_speed,
                        DasherPhase::Dash,
                        settings.dasher_dash_duration_s,
                    ),
                    DasherPhase::Dash => (
                        settings.dasher_dash_speed,
                        DasherPhase::Recover,
                        settings.dasher_recover_duration_s,
                    ),
                    DasherPhase::Recover => (
                        0.0,
                        DasherPhase::Approach,
                        settings.dasher_approach_duration_s,
                    ),
                };
                base.0 = dir * speed;
                state.phase_timer.tick(time.delta());
                if state.phase_timer.finished() {
                    state.phase = next_phase;
                    state.phase_timer = Timer::from_seconds(next_duration, TimerMode::Once);
                }
            }
            AiBehavior::Shooter => {
                let Some(mut state) = shooter_state else {
                    // Defensive: a shooter without ShooterState
                    // shouldn't exist (spawn_shooter always
                    // attaches one) but skipping is safer than
                    // panicking.
                    continue;
                };
                let enemy_pos = enemy_transform.translation.truncate();
                let to_player = player_pos - enemy_pos;
                let dist = to_player.length();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };

                // Approach until at preferred range, then stop.
                // The "stop" is just `speed = 0.0` — the enemy
                // doesn't need a separate state for "in range",
                // the distance check is enough. This keeps the
                // Shooter AI stateless (only the fire timer is
                // tracked, and that's per-entity on
                // `ShooterState`).
                let speed = if dist > settings.shooter_preferred_range {
                    settings.shooter_move_speed
                } else {
                    0.0
                };
                base.0 = dir * speed;

                // Tick the fire timer always. Only spawn a
                // projectile when (a) the timer elapsed AND
                // (b) we're in range. We deliberately do NOT
                // reset the timer if we're out of range —
                // `fire_timer.finished()` stays true, so the
                // moment we enter range we fire on the next
                // frame. The cap is implicit: a shooter that's
                // out of range for 5 seconds doesn't burst-fire
                // when it finally gets in range; it fires once
                // and resumes the normal cadence.
                state.fire_timer.tick(time.delta());
                if dist <= settings.shooter_preferred_range && state.fire_timer.finished() {
                    spawn_shooter_projectile(&mut commands, &settings, enemy_pos, dir);
                    let interval = 1.0 / settings.shooter_fire_rate;
                    state.fire_timer = Timer::from_seconds(interval, TimerMode::Once);
                }
            }
            AiBehavior::Sniper => {
                let Some(mut state) = sniper_state else {
                    // Defensive: a sniper without SniperState
                    // shouldn't exist (spawn_sniper always
                    // attaches one) but skipping is safer than
                    // panicking.
                    continue;
                };
                let enemy_pos = enemy_transform.translation.truncate();
                let to_player = player_pos - enemy_pos;
                let dist = to_player.length();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };

                // Sniper movement: approach until at preferred
                // range, then plant and charge. While in
                // cooldown (between shots), the sniper still
                // moves toward the player so it can re-plant
                // for the next charge — but slower than its
                // approach speed, so a determined player can
                // keep pushing during the cooldown window.
                let speed = if dist > settings.sniper_preferred_range {
                    settings.sniper_move_speed
                } else {
                    0.0
                };
                base.0 = dir * speed;

                // Charge / fire / cooldown state machine.
                //
                //   !is_charging, timer not finished → in
                //     cooldown after a recent shot, keep
                //     ticking the timer (and approaching the
                //     player if needed)
                //   !is_charging, timer finished → cooldown
                //     done, transition to charging
                //   is_charging, timer not finished → mid-
                //     charge, tick the timer, update the
                //     sprite color telegraph (lerp from
                //     SNIPER_COLOR to SNIPER_CHARGED_COLOR
                //     over the charge duration)
                //   is_charging, timer finished → fire the
                //     bullet, transition to cooldown
                state.charge_timer.tick(time.delta());

                if !state.is_charging {
                    // Cooldown phase
                    if state.charge_timer.finished() {
                        // Transition: cooldown done, start
                        // charging. Reset the timer to the
                        // full charge duration.
                        state.is_charging = true;
                        state.charge_timer =
                            Timer::from_seconds(settings.sniper_charge_duration_s, TimerMode::Once);
                    }
                    // While in cooldown, the sprite stays
                    // at SNIPER_COLOR (no telegraph).
                } else {
                    // Charging phase. Lerp the sprite color
                    // from base yellow to charged red over
                    // the charge duration — the visual
                    // telegraph that a shot is incoming.
                    let dur = state.charge_timer.duration().as_secs_f32();
                    if dur > 0.0 {
                        let progress =
                            (1.0 - state.charge_timer.remaining_secs() / dur).clamp(0.0, 1.0);
                        let base_srgba = SNIPER_COLOR.to_srgba();
                        let charged_srgba = SNIPER_CHARGED_COLOR.to_srgba();
                        sprite.color = Color::srgba(
                            base_srgba.red + (charged_srgba.red - base_srgba.red) * progress,
                            base_srgba.green + (charged_srgba.green - base_srgba.green) * progress,
                            base_srgba.blue + (charged_srgba.blue - base_srgba.blue) * progress,
                            base_srgba.alpha,
                        );
                    }
                    // When cooldown ends and we transition
                    // back to !is_charging, the sprite stays
                    // at the post-fire value (charged red)
                    // for one frame, then snaps back to
                    // base yellow on the next frame's "while
                    // in cooldown, sprite stays at base"
                    // branch. (We don't explicitly reset it
                    // here — the next frame's branch handles
                    // it because the visual only matters
                    // when is_charging is true.)
                    // Actually we DO need to reset it: the
                    // cooldown should show the sniper at
                    // base color, not stuck at red. Add the
                    // reset.
                    if state.charge_timer.finished() {
                        // Fire! Spawn a sniper projectile
                        // aimed at the player and transition
                        // to cooldown. We aim at the
                        // CURRENT player position (not the
                        // position when the charge started)
                        // — the sniper re-aims each frame
                        // during the charge (handled in the
                        // AI tick above, not here, since
                        // `dir` is recomputed each frame).
                        spawn_sniper_projectile(&mut commands, &settings, enemy_pos, dir);
                        state.is_charging = false;
                        state.charge_timer = Timer::from_seconds(
                            settings.sniper_post_fire_cooldown_s,
                            TimerMode::Once,
                        );
                        // Reset sprite to base yellow so
                        // the cooldown phase doesn't show
                        // a red sniper between shots.
                        sprite.color = SNIPER_COLOR;
                    }
                }
            }
            AiBehavior::Boss => {
                let Some(mut state) = boss_state else {
                    continue;
                };
                let enemy_pos = enemy_transform.translation.truncate();
                let to_player = player_pos - enemy_pos;
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };

                // Sync the mirrored HP first so the phase
                // transition below can read it.
                state.current_hp = health.current;

                // Phase detection: switch from Phase1 to
                // Phase2 the frame HP crosses the half-HP
                // threshold. We carry `current_hp` on the
                // state component (mirroring `Health.current`)
                // so the transition triggers on the
                // crossing, not on every frame the boss is
                // at low HP. Reset the attack timer so the
                // first Phase 2 burst doesn't fire on the
                // enrage frame.
                let half_hp = settings.boss_max_hp * 0.5;
                if state.phase == BossPhase::Phase1 && state.current_hp <= half_hp {
                    state.phase = BossPhase::Phase2;
                    state.attack_timer = Timer::from_seconds(
                        settings.boss_phase2_attack_interval_s,
                        TimerMode::Once,
                    );
                }

                // Movement: chase the player. Phase 1 at
                // base speed, Phase 2 at multiplier speed.
                let speed = match state.phase {
                    BossPhase::Phase1 => settings.boss_move_speed,
                    BossPhase::Phase2 => {
                        settings.boss_move_speed * settings.boss_phase2_speed_multiplier
                    }
                };
                base.0 = dir * speed;

                // Phase 2: 3-burst spread every interval.
                // Tick the timer; on finish, fire a 3-round
                // fan aimed at the current player pos, then
                // reset. The burst is a 17° spread (same
                // arc as the shotgun) so a player running
                // perpendicular dodges cleanly.
                if state.phase == BossPhase::Phase2 {
                    state.attack_timer.tick(time.delta());
                    if state.attack_timer.finished() {
                        let n = settings.boss_phase2_burst_count.max(1) as i32;
                        let total_spread: f32 = 0.30; // ~17°
                        for i in 0..n {
                            let t = if n == 1 {
                                0.0
                            } else {
                                i as f32 / (n - 1) as f32 - 0.5
                            };
                            let angle_offset = t * total_spread;
                            let (s, c) = angle_offset.sin_cos();
                            let burst_dir = Vec2::new(dir.x * c - dir.y * s, dir.x * s + dir.y * c);
                            spawn_boss_projectile(&mut commands, &settings, enemy_pos, burst_dir);
                        }
                        state.attack_timer = Timer::from_seconds(
                            settings.boss_phase2_attack_interval_s,
                            TimerMode::Once,
                        );
                    }
                }
            }
            // Phase 2 P5: Beacon is a static enemy (no movement)
            // that extends the Cascade window on kill. The
            // `Beacon` marker on the entity is what the kill
            // handler in `combat.rs` reads via `Has<Beacon>` to fire
            // the cascade-chain-extend hook; the contact-damage
            // handler skips the beacon via the same marker (see the
            // early-continue in `enemy_contact_damage_system`).
            //
            // Phase 2 P5.1: the beacon now FLEES from the player
            // instead of sitting still. It runs at
            // `beacon_flee_speed` directly away from the player's
            // position. During Cascade, the beacon is NOT
            // CascadeAffected, so it moves at full speed while
            // other enemies are slowed to 0.25× — the player
            // has to chase it down to get the chain extend.
            AiBehavior::Beacon => {
                            let to_player = player_pos - enemy_transform.translation.truncate();
                            let dir = if to_player.length_squared() > 0.0 {
                                -to_player.normalize()
                            } else {
                                Vec2::ZERO
                            };
                            base.0 = dir * settings.beacon_flee_speed;
                                        }
                                        AiBehavior::Orbiter => {
                                            let Some(mut state) = orbiter_state else {
                                                // Defensive: an orbiter without OrbiterState
                                                // shouldn't exist (spawn_orbiter always
                                                // attaches one) but skipping is safer than
                                                // panicking.
                                                continue;
                                            };
                                            let enemy_pos = enemy_transform.translation.truncate();
                                            let dt = time.delta().as_secs_f32();

                                            // Compute desired orbit position relative to the
                                            // player: a point on the orbit circle at the
                                            // current angle.
                                            let (sin, cos) = state.angle.sin_cos();
                                            let orbit_pos = player_pos + Vec2::new(cos, sin) * state.orbit_radius;

                                            // Move toward the orbit position.
                                            let to_orbit = orbit_pos - enemy_pos;
                                            let dir = if to_orbit.length_squared() > 0.0 {
                                                to_orbit.normalize()
                                            } else {
                                                Vec2::ZERO
                                            };
                                            base.0 = dir * settings.orbiter_move_speed;

                                            // Advance the orbit angle.
                                            state.angle += state.angular_speed * dt;
                                            if state.angle > std::f32::consts::TAU {
                                                state.angle -= std::f32::consts::TAU;
                                            }

                                            // Tick the fire timer. On finish, fire a projectile
                                            // tangent to the orbit: the tangent is perpendicular
                                            // to the angle vector (rotated +90°).
                                            state.fire_timer.tick(time.delta());
                                            if state.fire_timer.finished() {
                                                // Tangent direction: rotate the (cos, sin)
                                                // direction vector by +90° (counter-clockwise).
                                                let tangent = Vec2::new(-sin, cos);
                                                spawn_orbiter_projectile(&mut commands, &settings, enemy_pos, tangent);
                                                state.fire_timer = Timer::from_seconds(
                                                    1.0 / settings.orbiter_fire_rate,
                                                    TimerMode::Repeating,
                                                );
                                            }
                                        }
                                    }
                                }
                            }

