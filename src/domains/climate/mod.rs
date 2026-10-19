pub mod config;
pub mod data;
pub mod solver;
pub mod ui;

use crate::{
    domains::tectonics::data::GeologyLayer,
    world::{
        pipeline::{PipelineSet, WorldPipeline},
        topology::WorldTopology,
    },
};
use bevy::prelude::*;
use config::ClimateConfig;
use data::ClimateLayer;
use solver::simulate_all_seasons;
use ui::{draw_ocean_currents_gizmos, draw_wind_arrows_gizmos};

pub struct ClimatePlugin;

impl Plugin for ClimatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClimateConfig>()
            .init_resource::<ClimateLayer>()
            .add_systems(Update, run_climate_pipeline.in_set(PipelineSet::Simulation))
            .add_systems(
                Update,
                (draw_wind_arrows_gizmos, draw_ocean_currents_gizmos).in_set(PipelineSet::Render),
            );
    }
}

fn run_climate_pipeline(
    topology: Res<WorldTopology>,
    geology: Res<GeologyLayer>,
    config: Res<ClimateConfig>,
    mut climate: ResMut<ClimateLayer>,
    mut pipeline: ResMut<WorldPipeline>,
) {
    if pipeline.regenerate_climate {
        climate.resize(topology.cells.len());
        simulate_all_seasons(&topology, &geology, &mut climate, &config);
        pipeline.regenerate_climate = false;
        pipeline.rebuild_mesh = true;
    }
}
