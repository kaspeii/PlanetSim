// /src/domains/generator.rs

use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task},
};
use futures_lite::future;
use noise::{Fbm, Perlin};
use rand::RngExt;

use crate::{
    domains::{
        climate::{ClimateConfig, climate_data::ClimateLayer, simulate_all_seasons},
        tectonics::{
            TectonicsConfig, tectonics_data::GeologyLayer,
            generate_hotspots, generate_plates, simulate_tectonics,
        },
    },
    world::{
        pipeline::{PipelineStage, WorldPipeline},
        states::AppState,
        topology::WorldTopology,
    },
};

#[derive(Component)]
pub struct GenerationTask {
    pub stage: PipelineStage,
    pub task: Task<(Option<GeologyLayer>, ClimateLayer)>,
}

/// Вспомогательное условие для запуска систем в любом подсостоянии Generating(_)
pub fn is_generating(state: Res<State<AppState>>) -> bool {
    matches!(state.get(), AppState::Generating(_))
}

pub fn spawn_generation_task(
    mut commands: Commands,
    state: Res<State<AppState>>,
    topology: Res<WorldTopology>,
    geology_res: Res<GeologyLayer>,
    t_config: Res<TectonicsConfig>,
    c_config: Res<ClimateConfig>,
) {
    let AppState::Generating(stage) = *state.get() else { return; };

    let thread_pool = AsyncComputeTaskPool::get();
    let topology = (*topology).clone();
    let t_config = (*t_config).clone();
    let c_config = (*c_config).clone();

    // Если мы пересчитываем ТОЛЬКО климат, клонируем уже готовую геологию
    let existing_geology = if stage > PipelineStage::Tectonics {
        Some((*geology_res).clone())
    } else {
        None
    };

    let task = thread_pool.spawn(async move {
        // 1. Геология: либо считаем с нуля, либо берем существующую
        let (geology_for_climate, new_geology) = if let Some(geo) = existing_geology {
            (geo, None)
        } else {
            let mut rng = rand::rng();
            let seed = rng.random_range(0..=u32::MAX);
            let perlin = Fbm::<Perlin>::new(seed);

            let mut geology = GeologyLayer::default();
            geology.resize(topology.cells.len());
            geology.plates = generate_plates(&mut rng, &t_config);
            geology.volcanoes = generate_hotspots(&mut rng, &geology.plates, &t_config);
            simulate_tectonics(&topology, &mut geology, &perlin, &t_config);

            let geo_clone = geology.clone();
            (geo_clone, Some(geology))
        };

        // 2. Климат: считается всегда (т.к. зависит от тектоники или вызван напрямую)
        let mut climate = ClimateLayer::default();
        climate.resize(topology.cells.len());
        simulate_all_seasons(&topology, &geology_for_climate, &mut climate, &c_config);

        (new_geology, climate)
    });

    commands.spawn(GenerationTask { stage, task });
}

pub fn handle_generation_task(
    mut commands: Commands,
    mut task_query: Query<(Entity, &mut GenerationTask)>,
    mut geology_res: ResMut<GeologyLayer>,
    mut climate_res: ResMut<ClimateLayer>,
    mut pipeline: ResMut<WorldPipeline>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let Some((entity, mut gen_task)) = task_query.iter_mut().next() else { return; };

    if let Some((opt_geology, gen_climate)) = future::block_on(future::poll_once(&mut gen_task.task)) {
        // Если пересчитывалась тектоника — обновляем её и перестраиваем весь 3D-меш
        if let Some(gen_geology) = opt_geology {
            *geology_res = gen_geology;
            pipeline.rebuild_mesh = true;
        } else {
            // Если пересчитывался только климат — рельеф тот же, просто меняем цвета!
            pipeline.recolor_mesh = true;
        }

        *climate_res = gen_climate;

        next_state.set(AppState::InGame);
        commands.entity(entity).despawn();
    }
}