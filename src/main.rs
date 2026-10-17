mod config;
mod grid;
mod interaction;
mod mesh;
mod tectonics;

use bevy::prelude::*;
use config::PlanetConfig;
use grid::WorldGrid;
use interaction::InteractionPlugin;
use mesh::build_hex_mesh;
use noise::{Fbm, Perlin};
use rand::RngExt;
use tectonics::{generate_plates, generate_hotspots, simulate_tectonics};

#[derive(Component)]
pub struct Globe;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<PlanetConfig>()
        .add_plugins(PlanetPlugin)
        .add_plugins(InteractionPlugin)
        .run();
}

pub struct PlanetPlugin;

impl Plugin for PlanetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_globe);
    }
}

fn setup_globe(
    mut commands: Commands,
    config: Res<PlanetConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut grid = WorldGrid::new(config.subdivisions);

    let mut rng = rand::rng();
    let seed = rng.random_range(0..=u32::MAX);
    let perlin = Fbm::<Perlin>::new(seed);

    let plates = generate_plates(&mut rng, &config);
    let volcanoes = generate_hotspots(&mut rng, &plates, &config);
    simulate_tectonics(&mut grid, &plates, &volcanoes, &perlin, &config);

    let planet_mesh = build_hex_mesh(&grid, config.radius);
    commands.insert_resource(grid);

    let material_handle = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(planet_mesh)),
        MeshMaterial3d(material_handle),
        Globe,
        interaction::GlobeInertia::default(),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 12000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 9.5),
    ));
}   