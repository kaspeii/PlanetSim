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
            let temp = climate.temperatures[i];

            // 1. Снег и лед (если температура ниже замерзания)
            if temp < -5.0 {
                return Color::srgb(0.9, 0.95, 1.0).to_linear().to_f32_array();
            }

            // 2. Океан (вода всегда красится в синий, если не покрыта льдом)
            if h <= 0.0 {
                // Можно менять оттенок в зависимости от глубины
                let depth = (h / -1.0).clamp(0.0, 1.0);
                return Color::srgb(0.02, 0.1, 0.3 + (1.0 - depth) * 0.2)
                    .to_linear()
                    .to_f32_array();
            }

            // 3. Базовая суша (в будущем здесь будет чтение из массива биомов)
            if h < 0.035 {
                Color::srgb(0.85, 0.75, 0.5).to_linear().to_f32_array() // Низины (зеленые)
            } else if h < 0.2 {
                Color::srgb(0.2, 0.45, 0.15).to_linear().to_f32_array() // Низины (зеленые)
            } else {
                Color::srgb(0.4, 0.35, 0.3).to_linear().to_f32_array() // Горы (коричневые)
            }
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
