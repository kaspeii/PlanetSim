pub mod pipeline;
pub mod topology;
pub mod states;

use bevy::prelude::*;
use pipeline::{PipelineSet, WorldPipeline};
use topology::WorldTopology;
use states::AppState;

#[derive(Resource, Clone)]
pub struct WorldConfig {
    /// Радиус планеты в мировых координатах
    pub radius: f32,
    /// Степень детализации (сабдивы) икосаэдра (влияет на плотность сетки гексов)
    pub subdivisions: u32,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            radius: 5.0,     // release - 7
            subdivisions: 7, //release - 8
        }
    }
}

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldConfig>()
            .init_resource::<WorldPipeline>()
            .init_state::<AppState>()
            .configure_sets(
                Update,
                (
                    PipelineSet::Input,
                    PipelineSet::Simulation,
                    PipelineSet::Render,
                )
                    .chain(),
            )
            .add_systems(PreStartup, setup_topology);
    }
}

fn setup_topology(mut commands: Commands, config: Res<WorldConfig>) {
    let topology = WorldTopology::new(config.subdivisions);
    commands.insert_resource(topology);
}
