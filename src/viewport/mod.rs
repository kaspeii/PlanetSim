pub mod camera_3d;
pub mod mesh_globe;

use crate::{
    domains::{
        climate::{data::ClimateLayer, ui::build_temperature_colors},
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
use mesh_globe::{build_globe_mesh, recolor_globe_mesh};

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
    Elevation,
    TectonicPlates,
    Temperature,
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
        MapOverlay::Elevation => build_elevation_colors(&geology),
        MapOverlay::TectonicPlates => build_plate_colors(&geology),
        MapOverlay::Temperature => build_temperature_colors(&climate),
    };

if let Ok(mesh_handle) = query.single() {
        if let Some(mut mesh) = meshes.get_mut(&mesh_handle.0) {
            if pipeline.rebuild_mesh {
                *mesh = build_globe_mesh(&topology, &geology.elevations, &colors, world_config.radius);
            } else if pipeline.recolor_mesh {
                let vertex_colors = recolor_globe_mesh(
                    &topology,
                    &geology.elevations,
                    &colors
                );
                mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vertex_colors);
            }
        }
    }

    pipeline.rebuild_mesh = false;
    pipeline.recolor_mesh = false;
}
