use crate::{config::PlanetConfig, grid::WorldGrid, mesh::build_hex_mesh, tectonics::step_elevation_diffusion, Globe};
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
};

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (rotate_globe, zoom_camera, interactive_ca_step));
    }
}

#[derive(Component, Default)]
pub struct GlobeInertia {
    pub velocity: Vec2,
}

fn rotate_globe(
    mut query: Query<(&mut Transform, &mut GlobeInertia), With<Globe>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    accumulated_mouse: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    if dt == 0.0 { return; }

    if let Ok((mut transform, mut inertia)) = query.single_mut() {
        let sensitivity = 0.005;

        if mouse_button.pressed(MouseButton::Left) {
            let current_velocity = accumulated_mouse.delta / dt;
            inertia.velocity = inertia.velocity.lerp(current_velocity, 15.0 * dt);
        } else {
            let friction = 3.0;
            inertia.velocity *= (-friction * dt).exp();
            if inertia.velocity.length() < 1.0 {
                inertia.velocity = Vec2::ZERO;
            }
        }

        if inertia.velocity != Vec2::ZERO {
            let frame_delta = inertia.velocity * dt;
            let yaw = Quat::from_rotation_y(frame_delta.x * sensitivity);
            let pitch = Quat::from_rotation_x(frame_delta.y * sensitivity);

            transform.rotation = yaw * pitch * transform.rotation;
            transform.rotation = transform.rotation.normalize();
        }
    }
}

fn zoom_camera(
    mut query: Query<&mut Transform, With<Camera3d>>,
    scroll: Res<AccumulatedMouseScroll>,
) {
    if scroll.delta.y != 0.0 {
        if let Ok(mut transform) = query.single_mut() {
            let zoom_speed = 0.5;
            transform.translation.z = (transform.translation.z - scroll.delta.y * zoom_speed).clamp(4.5, 20.0);
        }
    }
}

fn interactive_ca_step(
    keys: Res<ButtonInput<KeyCode>>,
    mut grid: ResMut<WorldGrid>,
    config: Res<PlanetConfig>,
    query: Query<&Mesh3d, With<Globe>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if keys.just_pressed(KeyCode::Space) {
        if let Ok(mesh_handle) = query.single() {
            step_elevation_diffusion(&mut grid, 0.25);
            if let Some(mut mesh) = meshes.get_mut(&mesh_handle.0) {
                *mesh = build_hex_mesh(&grid, config.radius);
            }
        }
    }
}