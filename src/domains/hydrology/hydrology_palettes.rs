// src/domains/hydrology/hydrology_palettes.rs

use crate::{
    domains::tectonics::tectonics_data::GeologyLayer,
    viewport::{Globe, ViewportViewConfig},
    world::{WorldConfig, topology::WorldTopology},
};
use bevy::prelude::*;

use super::hydrology_data::{BioResource, Biome, HydrologyLayer, MineralResource, SoilType};
use bevy::color::{Color, ColorToComponents};

pub fn build_soil_moisture_colors(
    geology: &crate::domains::tectonics::tectonics_data::GeologyLayer,
    hydrology: &HydrologyLayer,
) -> Vec<[f32; 4]> {
    hydrology
        .soil_moisture
        .iter()
        .enumerate()
        .map(|(i, &sm)| {
            if geology.elevations[i] <= 0.0 {
                return Color::srgb(0.06, 0.12, 0.24).to_linear().to_f32_array();
            }
            let m = sm.clamp(0.0, 1.0);
            let (r, g, b) = if m < 0.25 {
                // Сухая почва: от раскаленно-бежевого к сухому желтому
                let t = m / 0.25;
                (0.88 - t * 0.10, 0.68 + t * 0.10, 0.38 - t * 0.05)
            } else if m < 0.55 {
                // Умеренно влажная почва: от желто-зеленого к насыщенному изумрудному
                let t = (m - 0.25) / 0.30;
                (0.78 - t * 0.55, 0.78 - t * 0.08, 0.33 + t * 0.12)
            } else {
                // Сильно увлажненная и пойменная почва: от бирюзового к глубокому синему
                let t = (m - 0.55) / 0.45;
                (0.23 - t * 0.15, 0.70 - t * 0.35, 0.45 + t * 0.48)
            };
            Color::srgb(r, g, b).to_linear().to_f32_array()
        })
        .collect()
}

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

/// Композитная карта продовольствия (Земледелие / Скотоводство / Сады и дичь / Рыболовство)
pub fn build_food_potential_colors(
    geology: &crate::domains::tectonics::tectonics_data::GeologyLayer,
    hydrology: &HydrologyLayer,
) -> Vec<[f32; 4]> {
    geology
        .elevations
        .iter()
        .enumerate()
        .map(|(i, &elev)| {
            let fish = hydrology.fishery_potential.get(i).copied().unwrap_or(0.0);
            if elev <= 0.0 {
                // В океане показываем богатство рыбных промыслов (шельфы и апвеллинг)
                let r = 0.04 + fish * 0.14;
                let g = 0.12 + fish * 0.58;
                let b = 0.26 + fish * 0.64;
                return Color::srgb(r, g, b).to_linear().to_f32_array();
            }

            let arable = hydrology.arable_potential.get(i).copied().unwrap_or(0.0);
            let pastoral = hydrology.pastoral_potential.get(i).copied().unwrap_or(0.0);
            let foraging = hydrology.foraging_potential.get(i).copied().unwrap_or(0.0);

            // Базовая бесплодная подложка (пустыня / ледник / скалы)
            let base_r = 0.24;
            let base_g = 0.22;
            let base_b = 0.20;

            // Arable -> Изумрудно-зеленый; Pastoral -> Золотисто-охристый; Foraging (Фрукты/Дичь) -> Ягодно-пурпурный/бирюзовый
            let r = (base_r + pastoral * 0.68 + foraging * 0.32 - arable * 0.10).clamp(0.05, 1.0);
            let g = (base_g + arable * 0.72 + pastoral * 0.38 + foraging * 0.20).clamp(0.05, 1.0);
            let b = (base_b + foraging * 0.58 + fish * 0.35 - pastoral * 0.12).clamp(0.05, 1.0);

            Color::srgb(r, g, b).to_linear().to_f32_array()
        })
        .collect()
}

/// Карта ископаемых ресурсов (15 типов минералов и энергоносителей)
pub fn build_mineral_colors(
    geology: &crate::domains::tectonics::tectonics_data::GeologyLayer,
    hydrology: &HydrologyLayer,
) -> Vec<[f32; 4]> {
    geology
        .elevations
        .iter()
        .enumerate()
        .map(|(i, &elev)| {
            let res = hydrology
                .minerals
                .get(i)
                .copied()
                .unwrap_or(MineralResource::None);
            let richness = hydrology
                .mineral_richness
                .get(i)
                .copied()
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);

            // Спокойная темно-серая подложка рельефа, чтобы месторождения ярко читались
            let bg = if elev <= 0.0 {
                [0.06, 0.11, 0.20]
            } else {
                let shade = (0.18 + elev.clamp(0.0, 0.5) * 0.25).clamp(0.16, 0.34);
                [shade, shade * 0.98, shade * 0.95]
            };

            if res == MineralResource::None {
                return Color::srgb(bg[0], bg[1], bg[2]).to_linear().to_f32_array();
            }

            let fg = match res {
                MineralResource::None => bg,
                MineralResource::Clay => [0.76, 0.48, 0.30], // Терракотовый
                MineralResource::StoneLimestone => [0.78, 0.80, 0.82], // Светло-мраморный
                MineralResource::SilicaQuartz => [0.60, 0.88, 0.92], // Кварцево-голубой
                MineralResource::Salt => [0.96, 0.96, 1.00], // Белоснежный
                MineralResource::Copper => [0.90, 0.45, 0.15], // Медно-рыжий
                MineralResource::Tin => [0.55, 0.68, 0.78],  // Оловянно-сизый
                MineralResource::Iron => [0.78, 0.18, 0.16], // Ржаво-красный гематит
                MineralResource::PreciousMetals => [1.00, 0.86, 0.12], // Ярко-золотой
                MineralResource::SulfurNitre => [0.92, 0.95, 0.18], // Серно-лимонный
                MineralResource::Coal => [0.12, 0.12, 0.14], // Антрацитово-черный
                MineralResource::Hydrocarbons => [0.52, 0.12, 0.68], // Нефтяной пурпурно-фиолетовый
                MineralResource::Bauxite => [0.94, 0.36, 0.32], // Бокситово-коралловый
                MineralResource::AlloyMetals => [0.24, 0.52, 0.92], // Кобальтово-синий (титан/никель)
                MineralResource::Uranium => [0.18, 0.98, 0.28],     // Ураново-неоновый зеленый
                MineralResource::RareEarths => [0.98, 0.18, 0.75],  // Ярко-маджентовый (РЗМ/литий)
            };

            let t = (0.45 + 0.55 * richness).clamp(0.0, 1.0);
            let r = bg[0] * (1.0 - t) + fg[0] * t;
            let g = bg[1] * (1.0 - t) + fg[1] * t;
            let b = bg[2] * (1.0 - t) + fg[2] * t;
            Color::srgb(r, g, b).to_linear().to_f32_array()
        })
        .collect()
}

/// Карта стратегических биоресурсов (Флора и Фауна, 9 типов)
pub fn build_bio_resource_colors(
    geology: &crate::domains::tectonics::tectonics_data::GeologyLayer,
    hydrology: &HydrologyLayer,
) -> Vec<[f32; 4]> {
    geology
        .elevations
        .iter()
        .enumerate()
        .map(|(i, &elev)| {
            let res = hydrology
                .bio_resources
                .get(i)
                .copied()
                .unwrap_or(BioResource::None);
            let richness = hydrology
                .bio_richness
                .get(i)
                .copied()
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);

            let bg = if elev <= 0.0 {
                [0.06, 0.12, 0.22]
            } else {
                let shade = (0.20 + elev.clamp(0.0, 0.5) * 0.20).clamp(0.18, 0.32);
                [shade * 0.95, shade, shade * 0.92]
            };

            if res == BioResource::None {
                return Color::srgb(bg[0], bg[1], bg[2]).to_linear().to_f32_array();
            }

            let fg = match res {
                BioResource::None => bg,
                BioResource::NavalTimber => [0.08, 0.48, 0.22], // Темно-изумрудный (корабельный лес)
                BioResource::TimberFuel => [0.36, 0.58, 0.28],  // Спокойный лесной зеленый
                BioResource::FiberCrops => [0.94, 0.94, 0.86],  // Хлопково-белый / льняной
                BioResource::ResinRubber => [0.16, 0.78, 0.72], // Смолисто-бирюзовый (каучук/смола)
                BioResource::LuxuryFlora => [0.88, 0.20, 0.52], // Пряно-карминовый (специи/чай/кофе)
                BioResource::Horses => [0.82, 0.48, 0.16],      // Гнедой / рыже-каштановый (лошади)
                BioResource::Camels => [0.94, 0.80, 0.36],      // Песочно-золотой (верблюды)
                BioResource::ElephantsDraft => [0.56, 0.46, 0.68], // Сланцево-лиловый (слоны/вьючные)
                BioResource::FurIvory => [0.68, 0.86, 0.98], // Арктический голубой (пушнина/кость)
            };

            let t = (0.45 + 0.55 * richness).clamp(0.0, 1.0);
            let r = bg[0] * (1.0 - t) + fg[0] * t;
            let g = bg[1] * (1.0 - t) + fg[1] * t;
            let b = bg[2] * (1.0 - t) + fg[2] * t;
            Color::srgb(r, g, b).to_linear().to_f32_array()
        })
        .collect()
}

pub(super) fn draw_river_flow_gizmos(
    mut gizmos: Gizmos,
    topology: Res<WorldTopology>,
    geology: Res<GeologyLayer>,
    hydrology: Res<HydrologyLayer>,
    view_config: Res<ViewportViewConfig>,
    world_config: Res<WorldConfig>,
    globe_query: Query<&Transform, With<Globe>>,
) {
    if !view_config.show_river_flow || hydrology.river_flow_arrows.is_empty() {
        return;
    }
    let Ok(globe_transform) = globe_query.single() else {
        return;
    };

    let r = world_config.radius;
    let base_len = r * 0.002;

    for &(i, dir, s) in &hydrology.river_flow_arrows {
        let cell_pos = topology.cells[i].center;
        let elev = geology.elevations[i].max(0.0);

        // Чуть выше поверхности — чтобы не тонуло в 3D-ленте реки.
        let local_start = cell_pos * (r + elev + r * 0.005);
        let arrow_len = base_len;
        let local_end = local_start + dir * arrow_len;

        let world_start = globe_transform.transform_point(local_start);
        let world_end = globe_transform.transform_point(local_end);

        // Голубой «вода-цвет», альфа растёт с мощностью реки.
        let alpha = 0.35 + 0.55 * s;
        let color = Color::srgb(1.0, 0.70 + 0.15 * s, 0.30 + 0.15 * s).with_alpha(alpha);

        gizmos.arrow(world_start, world_end, color).with_tip_length(arrow_len * 2.0);
    }
}