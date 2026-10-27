// src/domains/hydrology/hydrology_palettes.rs

use bevy::color::{Color, ColorToComponents};
use super::hydrology_data::{HydrologyLayer, SoilType};

pub fn build_soil_colors(hydrology: &HydrologyLayer) -> Vec<[f32; 4]> {
    hydrology
        .soils
        .iter()
        .map(|&soil| {
            let [r, g, b] = match soil {
                SoilType::Ocean => [0.08, 0.16, 0.30],     // Глубокий океанический синий
                SoilType::Leptosol => [0.52, 0.50, 0.48],  // Каменисто-серый (горные кручи)
                SoilType::Gelisol => [0.72, 0.80, 0.84],   // Сизо-ледяной (вечная мерзлота и тундра)
                SoilType::Podzol => [0.68, 0.64, 0.74],    // Пепельно-сиреневатый (таежные подзолы)
                SoilType::Cambisol => [0.62, 0.46, 0.28],  // Классический буро-коричневый (буроземы)
                SoilType::Chernozem => [0.22, 0.18, 0.15], // Глубокий угольно-коричневый (черноземы степей)
                SoilType::Aridisol => [0.90, 0.80, 0.55],  // Светло-песочный / охристый (пустыни)
                SoilType::Laterite => [0.82, 0.30, 0.18],  // Кирпично-красный (тропические латериты)
                SoilType::Andisol => [0.38, 0.22, 0.36],   // Темно-пурпурно-бурый (вулканические андисоли)
                SoilType::Gleysol => [0.30, 0.52, 0.46],   // Болотно-сизо-зеленый (пойменные и глеевые)
            };
            Color::srgb(r, g, b).to_linear().to_f32_array()
        })
        .collect()
}