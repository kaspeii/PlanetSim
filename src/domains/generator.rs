// /src/domains/generator.rs

use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
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
            GeologyLayer, TectonicsConfig,
            generate_hotspots, generate_plates, simulate_tectonics,
        },
    },
    world::{
        pipeline::{PipelineStage, WorldPipeline},
        topology::WorldTopology,
    },
};

/// Ресурс-запрос: позволяет UI или хоткеям заказать фоновую генерацию без блокировки экрана
#[derive(Resource, Default)]
pub struct GenerationRequest(pub Option<PipelineStage>);

impl GenerationRequest {
    pub fn request(&mut self, stage: PipelineStage) {
        self.0 = Some(stage);
    }
}

#[derive(Component)]
pub struct GenerationTask {
    pub step: Arc<AtomicU8>,
    pub total_steps: u8,
    pub task: Task<(Option<GeologyLayer>, ClimateLayer)>,
}

impl GenerationTask {
    /// Возвращает текст текущего этапа и прогресс от 0.0 до 1.0 для UI
    pub fn status(&self) -> (&'static str, f32) {
        let code = self.step.load(Ordering::Relaxed);
        let desc = match code {
            1 => "Формирование тектонических плит и вулканов...",
            2 => "Симуляция движения коры и орографии...",
            3 => "Расчет сезонов, ветров, течений и 3D-стрелок...",
            _ => "Подготовка потоков...",
        };
        // При пересчете только климата шаг сразу 3 из 1
        let progress = if self.total_steps == 1 {
            0.5
        } else {
            (code as f32 / self.total_steps as f32).clamp(0.0, 1.0)
        };
        (desc, progress)
    }
}

pub fn spawn_generation_task(
    mut commands: Commands,
    mut req: ResMut<GenerationRequest>,
    existing_tasks: Query<Entity, With<GenerationTask>>,
    topology: Res<WorldTopology>,
    geology_res: Res<GeologyLayer>,
    t_config: Res<TectonicsConfig>,
    c_config: Res<ClimateConfig>,
) {
    let Some(stage) = req.0.take() else { return; };

    // Если старая задача еще крутилась, отменяем её
    for entity in existing_tasks.iter() {
        commands.entity(entity).despawn();
    }

    let thread_pool = AsyncComputeTaskPool::get();
    let topology = (*topology).clone();
    let t_config = (*t_config).clone();
    let c_config = (*c_config).clone();

    let existing_geology = if stage > PipelineStage::Tectonics {
        Some((*geology_res).clone())
    } else {
        None
    };

    let step = Arc::new(AtomicU8::new(0));
    let step_clone = step.clone();
    let total_steps = if existing_geology.is_some() { 1 } else { 3 };

    let task = thread_pool.spawn(async move {
        let (geology_for_climate, new_geology) = if let Some(geo) = existing_geology {
            (geo, None)
        } else {
            // Шаг 1: Плиты и горячие точки
            step_clone.store(1, Ordering::Relaxed);
            let mut rng = rand::rng();
            let seed = rng.random_range(0..=u32::MAX);
            let perlin = Fbm::<Perlin>::new(seed);

            let mut geology = GeologyLayer::default();
            geology.resize(topology.cells.len());
            geology.plates = generate_plates(&mut rng, &t_config);
            geology.volcanoes = generate_hotspots(&mut rng, &geology.plates, &t_config);

            // Шаг 2: Тектоника и рельеф
            step_clone.store(2, Ordering::Relaxed);
            simulate_tectonics(&topology, &mut geology, &perlin, &t_config);

            let geo_clone = geology.clone();
            (geo_clone, Some(geology))
        };

        // Шаг 3: Климат и кэш стрелок
        step_clone.store(3, Ordering::Relaxed);
        let mut climate = ClimateLayer::default();
        climate.resize(topology.cells.len());
        simulate_all_seasons(&topology, &geology_for_climate, &mut climate, &c_config);

        (new_geology, climate)
    });

    commands.spawn(GenerationTask {
        step,
        total_steps,
        task,
    });
}

pub fn handle_generation_task(
    mut commands: Commands,
    mut task_query: Query<(Entity, &mut GenerationTask)>,
    mut geology_res: ResMut<GeologyLayer>,
    mut climate_res: ResMut<ClimateLayer>,
    mut pipeline: ResMut<WorldPipeline>,
) {
    let Some((entity, mut gen_task)) = task_query.iter_mut().next() else { return; };

    if let Some((opt_geology, gen_climate)) = future::block_on(future::poll_once(&mut gen_task.task)) {
        if let Some(gen_geology) = opt_geology {
            *geology_res = gen_geology;
            pipeline.rebuild_mesh = true;
        } else {
            pipeline.recolor_mesh = true;
        }

        *climate_res = gen_climate;
        commands.entity(entity).despawn();
    }
}