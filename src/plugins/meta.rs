//! Meta-progression plugin: save/load Crown currency, earn on extraction.

use bevy::prelude::*;
use log::info;
use std::fs;
use std::path::PathBuf;

use crate::components::{AppState, GameOverReason};
use crate::resources::{CrownSave, MetaConfig, RunState};

const SAVE_PATH: &str = "crown_save.ron";

pub struct MetaPlugin;

impl Plugin for MetaPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MetaConfig>()
            .add_systems(Startup, load_save)
            .add_systems(Update, on_extraction_earn_crown.run_if(in_state(AppState::GameOver)));
    }
}

fn save_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(SAVE_PATH)
}

fn load_save(mut commands: Commands) {
    let path = save_path();
    let save = if path.exists() {
        match fs::read_to_string(&path) {
            Ok(data) => ron::from_str(&data).unwrap_or_else(|e| {
                info!("Failed to parse Crown save ({}): starting fresh", e);
                CrownSave::default()
            }),
            Err(e) => {
                info!("Failed to read Crown save ({:?}): starting fresh", e);
                CrownSave::default()
            }
        }
    } else {
        info!("No Crown save found — starting fresh.");
        CrownSave::default()
    };
    info!("Crown save loaded: {} Crown, upgrades: {:?}", save.crown, save.upgrades);
    commands.insert_resource(save);
}

pub(crate) fn write_save(save: &CrownSave) {
    let path = save_path();
    match ron::to_string(save) {
        Ok(data) => {
            if let Err(e) = fs::write(&path, data) {
                info!("Failed to write Crown save: {:?}", e);
            }
        }
        Err(e) => info!("Failed to serialize Crown save: {:?}", e),
    }
}

/// Earn Crown on extraction (not on death).
fn on_extraction_earn_crown(
    run: Res<RunState>,
    mut save: ResMut<CrownSave>,
) {
    if run.game_over_reason != GameOverReason::Extracted {
        return;
    }
    let floor = run.biome_index.max(1);
    let gold = run.gold;
    let kills = run.enemies_killed;
    let earned = floor * 10 + gold / 5 + kills * 2 + 5;
    save.crown += earned;
    write_save(&save);
    info!("Extraction: earned {} Crown (total: {})", earned, save.crown);
}