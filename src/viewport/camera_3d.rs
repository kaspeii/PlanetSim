use super::{Globe, MapOverlay};
use crate::world::pipeline::WorldPipeline;
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
};

#[derive(Component, Default)]
pub struct GlobeInertia {
    pub velocity: Vec2,
}

pub fn rotate_globe(
    mut query: Query<(&mut Transform, &mut GlobeInertia), With<Globe>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    accumulated_mouse: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    if dt == 0.0 {
        return;
    }

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

pub fn zoom_camera(
    mut query: Query<&mut Transform, With<Camera3d>>,
    scroll: Res<AccumulatedMouseScroll>,
) {
    if scroll.delta.y != 0.0
        && let Ok(mut transform) = query.single_mut()
    {
        let zoom_speed = 0.5;
        transform.translation.z =
            (transform.translation.z - scroll.delta.y * zoom_speed).clamp(4.5, 20.0);
    }
}

pub fn hotkey_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut pipeline: ResMut<WorldPipeline>,
    mut overlay: ResMut<MapOverlay>,
) {
    // Шаг клеточного автомата сглаживания
    if keys.just_pressed(KeyCode::Space) {
        pipeline.step_tectonic_diffusion = true;
    }

    // Полная перегенерация планеты
    if keys.just_pressed(KeyCode::KeyR) {
        pipeline.regenerate_tectonics = true;
    }

    // Переключение режима карты (Рельеф <-> Плиты)
    if keys.just_pressed(KeyCode::Tab) {
        *overlay = match *overlay {
            MapOverlay::Elevation => MapOverlay::TectonicPlates,
            MapOverlay::TectonicPlates => MapOverlay::Elevation,
            MapOverlay::Temperature => MapOverlay::Temperature,
        };
        pipeline.rebuild_mesh = true;
    }
}
