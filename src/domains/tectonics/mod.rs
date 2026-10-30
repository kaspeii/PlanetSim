// src/domains/tectonics/mod.rs

pub mod earth_preset;
mod tectonics_config;
pub mod tectonics_data;
pub mod tectonics_palettes;
mod tectonics_solver;
pub mod tectonics_ui;

pub use earth_preset::build_earth_geology;
pub use tectonics_config::TectonicsConfig;
pub use tectonics_data::GeologyLayer;
pub use tectonics_solver::{generate_hotspots, generate_plates, simulate_tectonics};

use bevy::prelude::*;

pub struct TectonicsPlugin;

impl Plugin for TectonicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TectonicsConfig>()
            .init_resource::<GeologyLayer>();
    }
}