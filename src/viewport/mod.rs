// src/viewport/mod.rs

pub mod camera_3d;
pub mod hotkeys;
pub mod mesh_globe;

use crate::{
    domains::{
        climate::{
            climate_data::ClimateLayer,
            climate_palettes::{
                build_albedo_colors, build_continentality_colors, build_humidity_colors, build_precipitation_colors,
                build_pressure_colors, build_temperature_colors, build_wind_colors,
            },
        }, hydrology::{hydrology_data::HydrologyLayer, HydrologyConfig, hydrology_palettes::build_soil_colors}, tectonics::{
            tectonics_data::GeologyLayer, tectonics_palettes::{build_elevation_colors, build_lithology_colors, build_plate_colors},
        }
    }, world::{
        WorldConfig, pipeline::{PipelineSet, WorldPipeline}, topology::WorldTopology,
    },
};
use bevy::prelude::*;
use camera_3d::{rotate_globe, zoom_camera, GlobeInertia};
use hotkeys::hotkey_controls;
use mesh_globe::{build_globe_mesh, recolor_globe_mesh, reposition_globe_mesh, build_river_mesh};

#[derive(Component)]
pub struct Globe;

/// Режим проекции мира (задел под переключение между 3D-сферой и 2D-картой)
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum ViewMode {
    #[default]
    Globe3D,
    _Flat2D,
}

/// Активный слой раскраски карты (переключается по клавише Tab или из UI)
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum MapOverlay {
    #[default]
    SpaceView,
    Elevation,
    TectonicPlates,
    Lithology,
    Temperature,
    Humidity,
    Precipitation,
    Albedo,
    Continentality,
    Pressure,
    Winds,
    Soils,
}

#[derive(Component)]
pub struct RiverOverlayMesh;

#[derive(Resource)]
pub struct ViewportViewConfig {
    pub flat_water: bool,
    pub show_rivers: bool,
    pub show_wind_arrows: bool,
    pub show_ocean_currents: bool,
}

impl Default for ViewportViewConfig {
    fn default() -> Self {
        Self {
            flat_water: true,
            show_rivers: true,
            show_wind_arrows: false,
            show_ocean_currents: false,
        }
    }
}

pub struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewMode>()
            .init_resource::<MapOverlay>()
            .init_resource::<ViewportViewConfig>()
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

    let river_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.35,
        metallic: 0.05,
        ..default()
    });

    commands
        .spawn((
            Mesh3d(meshes.add(Mesh::from(Sphere::new(1.0)))),
            MeshMaterial3d(material_handle),
            Globe,
            GlobeInertia::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(Mesh::new(
                    bevy::render::mesh::PrimitiveTopology::TriangleList,
                    bevy::asset::RenderAssetUsages::default(),
                ))),
                MeshMaterial3d(river_material),
                RiverOverlayMesh,
            ));
        });

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

            let (base_r, base_g, base_b, base_albedo) = if h <= 0.0 {
                let depth = (h / -0.35).clamp(0.0, 1.0);
                let r = 0.02 + (1.0 - depth) * 0.04;
                let g = 0.10 + (1.0 - depth) * 0.18;
                let b = 0.28 + (1.0 - depth) * 0.25;
                (r, g, b, 0.06)
            } else if h < 0.03 {
                (0.76, 0.70, 0.50, 0.30)
            } else if h < 0.20 {
                let t = ((h - 0.03) / 0.17).clamp(0.0, 1.0);
                (0.20 + t * 0.12, 0.46 - t * 0.08, 0.15 + t * 0.05, 0.30)
            } else {
                (0.42, 0.37, 0.33, 0.30)
            };

            let snow_cover = ((albedo - base_albedo) / (0.80 - base_albedo)).clamp(0.0, 1.0);

            let r = base_r * (1.0 - snow_cover) + 0.92 * snow_cover;
            let g = base_g * (1.0 - snow_cover) + 0.96 * snow_cover;
            let b = base_b * (1.0 - snow_cover) + 1.00 * snow_cover;

            Color::srgb(r, g, b).to_linear().to_f32_array()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn sync_viewport_mesh(
    mut pipeline: ResMut<WorldPipeline>,
    topology: Res<WorldTopology>,
    world_config: Res<WorldConfig>,
    geology: Res<GeologyLayer>,
    climate: Res<ClimateLayer>,
    hydrology: Res<HydrologyLayer>,
    hydrology_config: Res<HydrologyConfig>,
    mut river_query: Query<(&Mesh3d, &mut Visibility), (With<RiverOverlayMesh>, Without<Globe>)>,
    overlay: Res<MapOverlay>,
    view_config: Res<ViewportViewConfig>,
    query: Query<&Mesh3d, With<Globe>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if let Ok((_, mut river_vis)) = river_query.single_mut() {
        let target_vis = if view_config.show_rivers {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *river_vis != target_vis {
            *river_vis = target_vis;
        }
    }

    if !pipeline.rebuild_mesh
        && !pipeline.reposition_mesh
        && !pipeline.recolor_mesh
        && !pipeline.rebuild_rivers
    {
        return;
    }

    if let Ok(mesh_handle) = query.single()
        && let Some(mut mesh) = meshes.get_mut(&mesh_handle.0)
    {
        let flatten_water = view_config.flat_water;

        if pipeline.rebuild_mesh {
            let colors = compute_overlay_colors(&overlay, &geology, &climate, &hydrology);
            *mesh = build_globe_mesh(
                &topology,
                &geology.elevations,
                flatten_water,
                &colors,
                world_config.radius,
            );
        } else {
            // Быстрый сдвиг вершин океана (при клике на "Плоский океан")
            if pipeline.reposition_mesh {
                let new_positions = reposition_globe_mesh(
                    &topology,
                    &geology.elevations,
                    flatten_water,
                    world_config.radius,
                );
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, new_positions);
            }

            // Быстрая смена палитры (при переключении радио-кнопок или сезона)
            if pipeline.recolor_mesh {
                let colors = compute_overlay_colors(&overlay, &geology, &climate, &hydrology);
                let vertex_colors = recolor_globe_mesh(&topology, &geology.elevations, &colors);
                mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vertex_colors);
            }
        }
    }

    if pipeline.rebuild_mesh || pipeline.reposition_mesh || pipeline.rebuild_rivers {
        if let Ok((river_mesh_handle, _)) = river_query.single()
            && let Some(mut r_mesh) = meshes.get_mut(&river_mesh_handle.0)
        {
            *r_mesh = build_river_mesh(
                &topology,
                &geology,
                &hydrology,
                &hydrology_config,
                view_config.flat_water,
                world_config.radius,
            );
        }
    }

    pipeline.rebuild_mesh = false;
    pipeline.reposition_mesh = false;
    pipeline.recolor_mesh = false;
    pipeline.rebuild_rivers = false;
}

fn compute_overlay_colors(
    overlay: &MapOverlay,
    geology: &GeologyLayer,
    climate: &ClimateLayer,
    hydrology: &HydrologyLayer,
) -> Vec<[f32; 4]> {
    match *overlay {
        MapOverlay::SpaceView => build_space_colors(geology, climate),
        MapOverlay::Elevation => build_elevation_colors(geology),
        MapOverlay::TectonicPlates => build_plate_colors(geology),
        MapOverlay::Lithology => build_lithology_colors(geology),
        MapOverlay::Temperature => build_temperature_colors(climate),
        MapOverlay::Humidity => build_humidity_colors(climate),
        MapOverlay::Precipitation => build_precipitation_colors(climate),
        MapOverlay::Albedo => build_albedo_colors(climate),
        MapOverlay::Continentality => build_continentality_colors(climate),
        MapOverlay::Pressure => build_pressure_colors(climate),
        MapOverlay::Winds => build_wind_colors(climate),
        MapOverlay::Soils => build_soil_colors(hydrology),
    }
}