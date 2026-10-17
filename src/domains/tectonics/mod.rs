pub mod config;
pub mod data;
pub mod solver;
pub mod ui;

use crate::world::{
    pipeline::{PipelineSet, WorldPipeline},
    topology::WorldTopology,
};
use bevy::prelude::*;
use config::TectonicsConfig;
use data::GeologyLayer;
use noise::{Fbm, Perlin};
use rand::RngExt;
use solver::{generate_hotspots, generate_plates, simulate_tectonics, step_elevation_diffusion};

pub struct TectonicsPlugin;

impl Plugin for TectonicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TectonicsConfig>()
            .init_resource::<GeologyLayer>()
            .add_systems(
                Update,
                run_tectonics_pipeline.in_set(PipelineSet::Simulation),
            );
    }
}

fn run_tectonics_pipeline(
    topology: Res<WorldTopology>,
    config: Res<TectonicsConfig>,
    mut geology: ResMut<GeologyLayer>,
    mut pipeline: ResMut<WorldPipeline>,
) {
    if pipeline.regenerate_tectonics {
        geology.resize(topology.len());

        let mut rng = rand::rng();
        let seed = rng.random_range(0..=u32::MAX);
        let perlin = Fbm::<Perlin>::new(seed);

        geology.plates = generate_plates(&mut rng, &config);
        geology.volcanoes = generate_hotspots(&mut rng, &geology.plates, &config);

        simulate_tectonics(&topology, &mut geology, &perlin, &config);

        pipeline.regenerate_tectonics = false;
        pipeline.rebuild_mesh = true;
    }

    if pipeline.step_tectonic_diffusion {
        step_elevation_diffusion(&topology, &mut geology.elevations, 0.25);
        pipeline.step_tectonic_diffusion = false;
        pipeline.rebuild_mesh = true;
    }
}
