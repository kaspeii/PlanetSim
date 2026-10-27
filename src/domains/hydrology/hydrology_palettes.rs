// src/domains/hydrology/hydrology_palettes.rs

use bevy::color::{Color, ColorToComponents};
use super::hydrology_data::{Biome, HydrologyLayer, SoilType};

pub fn build_soil_colors(hydrology: &HydrologyLayer) -> Vec<[f32; 4]> {
    hydrology
        .soils
        .iter()
        .map(|&soil| {
            let [r, g, b] = match soil {
                SoilType::Ocean => [0.08, 0.16, 0.30],
                SoilType::Leptosol => [0.52, 0.50, 0.48],
                SoilType::Gelisol => [0.72, 0.80, 0.84],
                SoilType::Podzol => [0.68, 0.64, 0.74],
                SoilType::Cambisol => [0.62, 0.46, 0.28],
                SoilType::Chernozem => [0.22, 0.18, 0.15],
                SoilType::Aridisol => [0.90, 0.80, 0.55],
                SoilType::Laterite => [0.82, 0.30, 0.18],
                SoilType::Andisol => [0.38, 0.22, 0.36],
                SoilType::Gleysol => [0.30, 0.52, 0.46],
            };
            Color::srgb(r, g, b).to_linear().to_f32_array()
        })
        .collect()
}

/// Четкая картографическая палитра для слоя MapOverlay::Biomes
pub fn build_biome_colors(hydrology: &HydrologyLayer) -> Vec<[f32; 4]> {
    hydrology
        .biomes
        .iter()
        .map(|&biome| {
            let [r, g, b] = match biome {
                // Океан
                Biome::DeepOcean => [0.05, 0.14, 0.32],
                Biome::ShallowOcean => [0.12, 0.34, 0.56],
                Biome::SeaIce => [0.85, 0.93, 0.98],

                // Холодные и горные
                Biome::IceSheet => [0.95, 0.98, 1.00],
                Biome::AlpineBare => [0.52, 0.49, 0.47],
                Biome::Tundra => [0.62, 0.70, 0.60],
                Biome::BorealTaiga => [0.14, 0.36, 0.24],

                // Умеренные
                Biome::TemperateDeciduousForest => [0.24, 0.56, 0.22],
                Biome::TemperateRainforest => [0.10, 0.46, 0.32],
                Biome::MediterraneanShrubland => [0.58, 0.56, 0.24],
                Biome::TemperateGrassland => [0.66, 0.76, 0.32],

                // Аридные
                Biome::ColdDesert => [0.76, 0.70, 0.58],
                Biome::HotDesert => [0.93, 0.80, 0.48],
                Biome::XericShrubland => [0.82, 0.66, 0.42],

                // Тропические
                Biome::Savanna => [0.76, 0.72, 0.26],
                Biome::TropicalSeasonalForest => [0.18, 0.62, 0.20],
                Biome::TropicalRainforest => [0.05, 0.38, 0.12],

                // Гидрологические азональные
                Biome::Wetland => [0.22, 0.48, 0.44],
                Biome::RiparianOasis => [0.20, 0.74, 0.36],
            };
            Color::srgb(r, g, b).to_linear().to_f32_array()
        })
        .collect()
}

/// Естественные спутниковые цвета биомов `(r, g, b, base_albedo)` для режима SpaceView ("Базовый")
#[inline]
pub fn biome_satellite_rgb(biome: Biome) -> (f32, f32, f32, f32) {
    match biome {
        Biome::DeepOcean | Biome::ShallowOcean => (0.03, 0.12, 0.30, 0.06),
        Biome::SeaIce => (0.88, 0.94, 0.99, 0.75),
        Biome::IceSheet => (0.94, 0.97, 1.00, 0.80),
        Biome::AlpineBare => (0.45, 0.41, 0.38, 0.28),
        Biome::Tundra => (0.48, 0.52, 0.40, 0.26),
        Biome::BorealTaiga => (0.12, 0.28, 0.16, 0.15),
        Biome::TemperateDeciduousForest => (0.20, 0.42, 0.16, 0.18),
        Biome::TemperateRainforest => (0.10, 0.34, 0.20, 0.15),
        Biome::MediterraneanShrubland => (0.46, 0.45, 0.22, 0.22),
        Biome::TemperateGrassland => (0.52, 0.58, 0.26, 0.24),
        Biome::ColdDesert => (0.68, 0.62, 0.50, 0.32),
        Biome::HotDesert => (0.84, 0.71, 0.46, 0.38),
        Biome::XericShrubland => (0.72, 0.58, 0.38, 0.30),
        Biome::Savanna => (0.58, 0.56, 0.24, 0.24),
        Biome::TropicalSeasonalForest => (0.15, 0.44, 0.14, 0.16),
        Biome::TropicalRainforest => (0.07, 0.30, 0.10, 0.13),
        Biome::Wetland => (0.18, 0.38, 0.30, 0.16),
        Biome::RiparianOasis => (0.18, 0.50, 0.22, 0.18),
    }
}