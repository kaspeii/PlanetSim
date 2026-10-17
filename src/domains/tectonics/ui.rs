use super::data::GeologyLayer;
use bevy::prelude::*;

/// Генерирует массив цветов по высотам рельефа
pub fn build_elevation_colors(geology: &GeologyLayer) -> Vec<[f32; 4]> {
    geology
        .elevations
        .iter()
        .map(|&h| elevation_to_color(h).to_linear().to_f32_array())
        .collect()
}

/// Генерирует массив цветов для режима просмотра тектонических плит (переключение по Tab)
pub fn build_plate_colors(geology: &GeologyLayer) -> Vec<[f32; 4]> {
    let total_plates = geology.plates.len().max(1) as f32;
    geology
        .plate_ids
        .iter()
        .map(|&plate_id| {
            let hue = (plate_id as f32 / total_plates) * 360.0;
            Color::hsl(hue, 0.65, 0.5).to_linear().to_f32_array()
        })
        .collect()
}

pub fn elevation_to_color(h: f32) -> Color {
    match h {
        x if x <= -0.45 => Color::srgb(0.0, 0.03, 0.12),
        x if x <= -0.18 => Color::srgb(0.01, 0.1, 0.3),
        x if x < 0.0 => Color::srgb(0.05, 0.25, 0.5),
        x if x < 0.035 => Color::srgb(0.85, 0.75, 0.5),
        x if x < 0.18 => Color::srgb(0.2, 0.45, 0.15),
        x if x < 0.4 => Color::srgb(0.4, 0.35, 0.3),
        x if x < 0.6 => Color::srgb(0.3, 0.25, 0.2),
        _ => Color::srgb(0.95, 0.95, 1.0),
    }
}
