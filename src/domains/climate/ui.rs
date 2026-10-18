use super::data::ClimateLayer;
use bevy::prelude::*;

pub fn build_temperature_colors(climate: &ClimateLayer) -> Vec<[f32; 4]> {
    climate
        .temperatures
        .iter()
        .map(|&t| temperature_to_color(t).to_linear().to_f32_array())
        .collect()
}

fn temperature_to_color(t: f32) -> Color {
    match t {
        x if x < -20.0 => Color::srgb(0.9, 0.9, 1.0), // Белый/Лед
        x if x < -5.0 => Color::srgb(0.6, 0.2, 0.8),  // Фиолетовый
        x if x < 5.0 => Color::srgb(0.0, 0.3, 0.8),   // Синий
        x if x < 15.0 => Color::srgb(0.2, 0.7, 0.4),  // Зеленый
        x if x < 25.0 => Color::srgb(0.9, 0.8, 0.2),  // Желтый
        x if x < 35.0 => Color::srgb(0.9, 0.4, 0.1),  // Оранжевый
        _ => Color::srgb(0.7, 0.1, 0.1),              // Красный
    }
}

pub fn build_albedo_colors(climate: &ClimateLayer) -> Vec<[f32; 4]> {
    climate
        .albedo
        .iter()
        .map(|&a| Color::srgb(a, a, a).to_linear().to_f32_array()) // От черного до белого
        .collect()
}

pub fn build_continentality_colors(climate: &ClimateLayer) -> Vec<[f32; 4]> {
    // Находим самую глубокую точку суши для нормализации цвета
    let max_dist = climate.distance_to_ocean.iter().copied().max().unwrap_or(1) as f32;
    climate
        .distance_to_ocean
        .iter()
        .map(|&d| {
            let val = 1.0 - (d as f32 / max_dist); // Берег - белый, глубь материка - черная
            Color::srgb(val, val, val).to_linear().to_f32_array()
        })
        .collect()
}

// Здесь же будет draw_climate_tab() для bevy_egui с ползунками из config
