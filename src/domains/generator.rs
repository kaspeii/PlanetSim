// src/domains/generator.rs

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
        climate::{ClimateConfig, climate_data::ClimateLayer, simulate_all_seasons}, hydrology::{HydrologyConfig, HydrologyLayer, simulate_hydrology, apply_surface_feedback_to_climate}, tectonics::{
            GeologyLayer, TectonicsConfig,
            generate_hotspots, generate_plates, simulate_tectonics,
        }
    }, world::{
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
    pub task: Task<(Option<GeologyLayer>, Option<ClimateLayer>, HydrologyLayer)>,
}

impl GenerationTask {
    pub fn status(&self) -> (&'static str, f32) {
        let code = self.step.load(Ordering::Relaxed);
        let desc = match code {
            1 => "Формирование тектонических плит и вулканов...",
            2 => "Симуляция движения коры и литологии...",
            3 => "Расчет сезонов, ветров, течений и осадков...",
            4 => "Формирование почв (CLORPT) и водного баланса...",
            5 => "Обратная связь (биомы и реки <=> Климат)",
            _ => "Подготовка потоков...",
        };
        let progress = (code as f32 / self.total_steps.max(1) as f32).clamp(0.0, 1.0);
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
    climate_res: Res<ClimateLayer>,
    h_config: Res<HydrologyConfig>,
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

    let step = Arc::new(AtomicU8::new(0));
    let step_clone = step.clone();

    let h_config = (*h_config).clone();

    let existing_geology = if stage > PipelineStage::Tectonics {
        Some((*geology_res).clone())
    } else {
        None
    };

    let existing_climate = if stage > PipelineStage::Climate {
        Some((*climate_res).clone())
    } else {
        None
    };

    let total_steps = match stage {
        PipelineStage::Tectonics => 4,
        PipelineStage::Climate => 2,
        PipelineStage::Hydrology => 1,
    };

    let coupled = h_config.coupled_climate_feedback;
    let total_steps = match stage {
        PipelineStage::Tectonics => if coupled { 5 } else { 4 },
        PipelineStage::Climate => if coupled { 3 } else { 2 },
        PipelineStage::Hydrology => if coupled { 2 } else { 1 },
    };

    let task = thread_pool.spawn(async move {
        // Шаг 1–2: Тектоника (если нужна)
        let (geology_ref, new_geology) = if let Some(geo) = existing_geology {
            (geo, None)
        } else {
            step_clone.store(1, Ordering::Relaxed);
            let mut rng = rand::rng();
            let seed = rng.random_range(0..=u32::MAX);
            let perlin = Fbm::<Perlin>::new(seed);

            let mut geology = GeologyLayer::default();
            geology.resize(topology.cells.len());
            geology.plates = generate_plates(&mut rng, &t_config);
            geology.volcanoes = generate_hotspots(&mut rng, &geology.plates, &t_config);

            step_clone.store(2, Ordering::Relaxed);
            simulate_tectonics(&topology, &mut geology, &perlin, &t_config);

            let geo_clone = geology.clone();
            (geo_clone, Some(geology))
        };

        let mut hydrology = HydrologyLayer::default();

        // Шаг 3–4: Климат и Гидрология
        let (climate_ref, new_climate) = if let Some(mut clim) = existing_climate {
            // Заказан пересчет со стадии Hydrology
            step_clone.store(4, Ordering::Relaxed);
            simulate_hydrology(&topology, &geology_ref, &clim, &mut hydrology, &h_config);

            if coupled {
                step_clone.store(5, Ordering::Relaxed);
                apply_surface_feedback_to_climate(&geology_ref, &mut clim, &hydrology, &c_config, &h_config);
                simulate_all_seasons(&topology, &geology_ref, &mut clim, &c_config);
                simulate_hydrology(&topology, &geology_ref, &clim, &mut hydrology, &h_config);
                let c_clone = clim.clone();
                (c_clone, Some(clim))
            } else {
                (clim, None)
            }
        } else {
            // Проход 1: Первичный климат и первичная гидрология/биомы
            step_clone.store(3, Ordering::Relaxed);
            let mut climate = ClimateLayer::default();
            climate.resize(topology.cells.len());
            climate.reset_surface_defaults(
                topology.cells.len(),
                c_config.land_albedo,
                c_config.land_moisture_recycling,
            );
            simulate_all_seasons(&topology, &geology_ref, &mut climate, &c_config);

            step_clone.store(4, Ordering::Relaxed);
            simulate_hydrology(&topology, &geology_ref, &climate, &mut hydrology, &h_config);

            // Проход 2: Обратная связь (Биомы + Реки + Почвы -> Альбедо + Испарение -> Климат -> Гидрология)
            if coupled {
                step_clone.store(5, Ordering::Relaxed);
                apply_surface_feedback_to_climate(
                    &geology_ref,
                    &mut climate,
                    &hydrology,
                    &c_config,
                    &h_config,
                );
                simulate_all_seasons(&topology, &geology_ref, &mut climate, &c_config);
                simulate_hydrology(&topology, &geology_ref, &climate, &mut hydrology, &h_config);
            }

            let clim_clone = climate.clone();
            (clim_clone, Some(climate))
        };

        let _ = climate_ref;
        (new_geology, new_climate, hydrology)
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
    mut hydrology_res: ResMut<HydrologyLayer>,
    mut pipeline: ResMut<WorldPipeline>,
) {
    let Some((entity, mut gen_task)) = task_query.iter_mut().next() else { return; };

    if let Some((opt_geology, opt_climate, gen_hydrology)) =
        future::block_on(future::poll_once(&mut gen_task.task))
    {
        if let Some(gen_geology) = opt_geology {
            *geology_res = gen_geology;
            pipeline.rebuild_mesh = true;
        } else {
            pipeline.recolor_mesh = true;
        }
        if let Some(gen_climate) = opt_climate {
            *climate_res = gen_climate;
        }
        *hydrology_res = gen_hydrology;
        pipeline.rebuild_rivers = true;
        commands.entity(entity).despawn();
    }
}
