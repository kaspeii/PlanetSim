pub mod camera_3d;
pub mod mesh_globe;

use crate::{
    domains::{
        climate::{
            data::ClimateLayer,
            ui::{
                build_albedo_colors, build_continentality_colors, build_pressure_colors,
                build_temperature_colors, build_wind_colors,
            },
        },
        tectonics::{
            data::GeologyLayer,
            ui::{build_elevation_colors, build_plate_colors},
        },
    },
    world::{
        WorldConfig,
        pipeline::{PipelineSet, WorldPipeline},
        topology::WorldTopology,
    },
};
use bevy::prelude::*;
use camera_3d::{GlobeInertia, hotkey_controls, rotate_globe, zoom_camera};
use mesh_globe::{build_globe_mesh, recolor_globe_mesh, reposition_globe_mesh};

#[derive(Component)]
pub struct Globe;

/// Режим проекции мира (задел под переключение между 3D-сферой и 2D-картой)
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum ViewMode {
    #[default]
    Globe3D,
    _Flat2D,
}

/// Активный слой раскраски карты (переключается по клавише Tab или из будущего UI)
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum MapOverlay {
    #[default]
    SpaceView,
    Elevation,
    TectonicPlates,
    Temperature,
    Albedo,
    Continentality,
    Pressure,
    Winds,
    // Humidity,
    // Biomes,
    // Civilizations,
}

pub struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewMode>()
            .init_resource::<MapOverlay>()
            .add_systems(Startup, setup_scene)
            .add_systems(
                Update,
                (rotate_globe, zoom_camera, hotkey_controls).in_set(PipelineSet::Input),
            )
            .add_systems(Update, sync_viewport_mesh.in_set(PipelineSet::Render));
    }
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let material_handle = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        ..default()
    });

    // Спавним сущность планеты с пустым мешем, который заполнится на первом кадре пайплайна
    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Sphere::new(1.0)))),
        MeshMaterial3d(material_handle),
        Globe,
        GlobeInertia::default(),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 12000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((Camera3d::default(), Transform::from_xyz(0.0, 0.0, 9.5)));
}

pub fn build_space_colors(geology: &GeologyLayer, climate: &ClimateLayer) -> Vec<[f32; 4]> {
    geology
        .elevations
        .iter()
        .enumerate()
        .map(|(i, &h)| {
            let albedo = climate.albedo[i];

            // 1. Считаем базовый цвет поверхности (без льда) и её родное альбедо
            let (base_r, base_g, base_b, base_albedo) = if h <= 0.0 {
                // Океан: на шельфе (близко к 0.0) вода чуть светлее и бирюзовее, в бездне — темно-синяя
                let depth = (h / -0.35).clamp(0.0, 1.0);
                let r = 0.02 + (1.0 - depth) * 0.04;
                let g = 0.10 + (1.0 - depth) * 0.18;
                let b = 0.28 + (1.0 - depth) * 0.25;
                (r, g, b, 0.06) // 0.06 — ocean_albedo
            } else if h < 0.03 {
                // Пляжи и прибрежные низины
                (0.76, 0.70, 0.50, 0.30)
            } else if h < 0.20 {
                // Равнины и предгорья (зелень)
                let t = ((h - 0.03) / 0.17).clamp(0.0, 1.0);
                (0.20 + t * 0.12, 0.46 - t * 0.08, 0.15 + t * 0.05, 0.30)
            } else {
                // Высокие горы (скалы)
                (0.42, 0.37, 0.33, 0.30)
            };

            // 2. Вычисляем долю снега/льда (0.0 — чисто, 1.0 — сплошной ледник с альбедо 0.8)
            let snow_cover = ((albedo - base_albedo) / (0.80 - base_albedo)).clamp(0.0, 1.0);

            // 3. Плавно смешиваем базовый рельеф со снежной шапкой (0.92, 0.96, 1.0)
            let r = base_r * (1.0 - snow_cover) + 0.92 * snow_cover;
            let g = base_g * (1.0 - snow_cover) + 0.96 * snow_cover;
            let b = base_b * (1.0 - snow_cover) + 1.00 * snow_cover;

            Color::srgb(r, g, b).to_linear().to_f32_array()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)] // не забыть нормально пофиксить при рефакторинге
fn sync_viewport_mesh(
    mut pipeline: ResMut<WorldPipeline>,
    topology: Res<WorldTopology>,
    world_config: Res<WorldConfig>,
    geology: Res<GeologyLayer>,
    climate: Res<ClimateLayer>,
    overlay: Res<MapOverlay>,
    query: Query<&Mesh3d, With<Globe>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if !pipeline.rebuild_mesh && !pipeline.recolor_mesh {
        return;
    }
    if geology.elevations.is_empty() || climate.temperatures.is_empty() {
        return;
    }

    let colors = match *overlay {
        MapOverlay::SpaceView => build_space_colors(&geology, &climate),
        MapOverlay::Elevation => build_elevation_colors(&geology),
        MapOverlay::TectonicPlates => build_plate_colors(&geology),
        MapOverlay::Temperature => build_temperature_colors(&climate),
        MapOverlay::Albedo => build_albedo_colors(&climate),
        MapOverlay::Continentality => build_continentality_colors(&climate),
        MapOverlay::Pressure => build_pressure_colors(&climate),
        MapOverlay::Winds => build_wind_colors(&climate),
    };

    if let Ok(mesh_handle) = query.single()
        && let Some(mut mesh) = meshes.get_mut(&mesh_handle.0)
    {
        let flatten_water = *overlay == MapOverlay::SpaceView;
        if pipeline.rebuild_mesh {
            *mesh = build_globe_mesh(
                &topology,
                &geology.elevations,
                flatten_water,
                &colors,
                world_config.radius,
            );
        } else if pipeline.recolor_mesh {
            // 1. Быстро обновляем высоты (сплющиваем или возвращаем океан)
            let new_positions = reposition_globe_mesh(
                &topology,
                &geology.elevations,
                flatten_water,
                world_config.radius,
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, new_positions);

            // 2. Быстро обновляем цвета (ВАЖНО: передаем всегда &geology.elevations, чтобы совпало число вершин)
            let vertex_colors = recolor_globe_mesh(&topology, &geology.elevations, &colors);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vertex_colors);
        }
    }

    pipeline.rebuild_mesh = false;
    pipeline.recolor_mesh = false;
}
