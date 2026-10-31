// src/domains/tectonics/tectonics_palettes.rs

use crate::domains::palettes::{rgb_u8, LegendEntry, MapLegend};
use super::tectonics_data::{GeologyLayer, PlateType, RockType};
use bevy::color::{Color, ColorToComponents};
use bevy::prelude::*;

pub fn build_lithology_colors(geology: &GeologyLayer) -> Vec<[f32; 4]> {
    geology
        .lithology
        .iter()
        .enumerate()
        .map(|(i, &rock)| {
            let elev = geology.elevations[i];
            let base_rgb = match rock {
                // Магматические и метаморфические
                RockType::Granite => [0.92, 0.48, 0.58], // Розово-коралловый (гранитные батолиты)
                RockType::GneissSchist => [0.64, 0.44, 0.74], // Лавандово-пурпурный (метаморфиты и щиты)
                RockType::Andesite => [0.88, 0.32, 0.18], // Ярко-терракотовый (вулканические дуги)
                RockType::FloodBasalt => [0.28, 0.25, 0.30], // Темный базальтовый (траппы, рифты, плюмы)

                // Осадочные суши и шельфа
                RockType::Sandstone => [0.88, 0.76, 0.44], // Песочно-золотистый (песчаники)
                RockType::ShaleClay => [0.52, 0.62, 0.45], // Оливково-серо-зеленый (глины и сланцы)
                RockType::Limestone => [0.48, 0.84, 0.82], // Светло-бирюзовый (известняки и рифы)

                // Океаническое ложа
                RockType::OceanicBasalt => [0.16, 0.22, 0.34], // Темно-сине-серый (молодой базальт MORB)
                RockType::PelagicSediment => [0.26, 0.36, 0.48], // Приглушенный сизо-синий (абиссальные илы)
            };

            // Слегка приглушаем яркость под водой, чтобы береговая линия читалась мгновенно
            let shade = if elev <= 0.0 { 0.72 } else { 1.0 };
            Color::srgb(
                base_rgb[0] * shade,
                base_rgb[1] * shade,
                base_rgb[2] * shade,
            )
            .to_linear()
            .to_f32_array()
        })
        .collect()
}

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
    // сколько всего континентальных и океанических
    let (mut n_cont, mut n_ocean) = (0usize, 0usize);
    for p in &geology.plates {
        match p.plate_type {
            PlateType::Continental => n_cont += 1,
            PlateType::Oceanic => n_ocean += 1,
        }
    }
    let n_cont = n_cont.max(1) as f32;
    let n_ocean = n_ocean.max(1) as f32;

    // индекс плиты внутри своего типа
    let mut cont_rank = vec![0usize; geology.plates.len()];
    let mut ocean_rank = vec![0usize; geology.plates.len()];
    let (mut ci, mut oi) = (0usize, 0usize);
    for (i, p) in geology.plates.iter().enumerate() {
        match p.plate_type {
            PlateType::Continental => {
                cont_rank[i] = ci;
                ci += 1;
            }
            PlateType::Oceanic => {
                ocean_rank[i] = oi;
                oi += 1;
            }
        }
    }

    geology
        .plate_ids
        .iter()
        .enumerate()
        .map(|(i, &plate_id)| {
            if geology.volcano_mask[i] {
                return Color::srgb(0.1, 0.1, 0.1).to_linear().to_f32_array();
            }

            let Some(plate) = geology.plates.get(plate_id) else {
                return Color::srgb(1.0, 1.0, 1.0).to_linear().to_f32_array();
            };

            let (hue, sat, light) = match plate.plate_type {
                PlateType::Continental => {
                    let t = cont_rank[plate_id] as f32 / n_cont;
                    (10.0 + t * 85.0, 0.75, 0.40)
                }
                PlateType::Oceanic => {
                    let t = ocean_rank[plate_id] as f32 / n_ocean;
                    (185.0 + t * 85.0, 0.55, 0.20)
                }
            };

            Color::hsl(hue, sat, light).to_linear().to_f32_array()
        })
        .collect()
}

pub fn elevation_to_color(h: f32) -> Color {
    match h {
        x if x <= -0.44 => Color::srgb(0.05, 0.02, 0.01), // почти чёрный кофе
        x if x <= -0.16 => Color::srgb(0.15, 0.08, 0.04), // тёмный шоколад
        x if x < -0.02 => Color::srgb(0.30, 0.17, 0.09),  // горький шоколад
        x if x < 0.01 => Color::srgb(0.50, 0.32, 0.17),   // карамель (было 0.035)
        x if x < 0.12 => Color::srgb(0.46, 0.29, 0.15),   // средний коричневый — равнина (светлее)
        x if x < 0.32 => Color::srgb(0.62, 0.40, 0.20),   // тёплый орех (было 0.18)
        x if x < 0.50 => Color::srgb(0.78, 0.46, 0.19),   // охра (было 0.4)
        x if x < 0.64 => Color::srgb(0.87, 0.33, 0.12),   // кирпич (было 0.55)
        _ => Color::srgb(0.95, 0.18, 0.10),               // раскалённый красный (было 0.7)
    }
}

pub fn build_ruggedness_colors(geology: &GeologyLayer) -> Vec<[f32; 4]> {
    geology
        .terrain_ruggedness
        .iter()
        .enumerate()
        .map(|(i, &rug)| {
            let elev = geology.elevations[i];
            let r_val = rug.clamp(0.0, 1.0);

            if elev <= 0.0 {
                // На морском дне показываем батиметрическую расчлененность (СОХ и желоба) в синих тонах
                let r = 0.04 + r_val * 0.18;
                let g = 0.10 + r_val * 0.28;
                let b = 0.22 + r_val * 0.35;
                return Color::srgb(r, g, b).to_linear().to_f32_array();
            }

            let (r, g, b) = if r_val < 0.25 {
                // Гладкие равнины: от темно-зеленого к оливково-желтому
                let t = r_val / 0.25;
                (0.18 + t * 0.52, 0.56 + t * 0.16, 0.24 - t * 0.06)
            } else if r_val < 0.60 {
                // Холмы, овраги и предгорья: от желтого к рыже-бурому
                let t = (r_val - 0.25) / 0.35;
                (0.70 + t * 0.22, 0.72 - t * 0.42, 0.18 - t * 0.06)
            } else {
                // Скалистые хребты и каньоны: от багрово-красного к сиренево-белому
                let t = (r_val - 0.60) / 0.40;
                (0.92 - t * 0.05, 0.30 - t * 0.10 + t * 0.55, 0.12 + t * 0.78)
            };

            Color::srgb(r, g, b).to_linear().to_f32_array()
        })
        .collect()
}

// ==== ЛЕГЕНДЫ КАРТЫ ====

/// Легенда рельефа — пороги взяты 1:1 из `elevation_to_color`.
pub fn elevation_legend() -> MapLegend {
    MapLegend {
        title: "Рельеф",
        entries: vec![
            LegendEntry { rgb: rgb_u8(0.05, 0.02, 0.01), label: "Абиссаль  (≤ −0.44)".into() },
            LegendEntry { rgb: rgb_u8(0.15, 0.08, 0.04), label: "Глубокий океан  (−0.44…−0.16)".into() },
            LegendEntry { rgb: rgb_u8(0.30, 0.17, 0.09), label: "Океан  (−0.16…−0.02)".into() },
            LegendEntry { rgb: rgb_u8(0.50, 0.32, 0.17), label: "Шельф / берег  (~0)".into() },
            LegendEntry { rgb: rgb_u8(0.46, 0.29, 0.15), label: "Равнины  (0.01…0.12)".into() },
            LegendEntry { rgb: rgb_u8(0.62, 0.40, 0.20), label: "Возвышенности  (0.12…0.32)".into() },
            LegendEntry { rgb: rgb_u8(0.78, 0.46, 0.19), label: "Предгорья  (0.32…0.50)".into() },
            LegendEntry { rgb: rgb_u8(0.87, 0.33, 0.12), label: "Горы  (0.50…0.64)".into() },
            LegendEntry { rgb: rgb_u8(0.95, 0.18, 0.10), label: "Пики  (≥ 0.64)".into() },
        ],
    }
}

/// Легенда литологии — перечисляем все типы пород, цвета совпадают с палитрой.
pub fn lithology_legend() -> MapLegend {
    let rocks: &[(RockType, &str, [f32; 3])] = &[
        (RockType::Granite,        "Гранит",              [0.92, 0.48, 0.58]),
        (RockType::GneissSchist,   "Гнейсы / сланцы",     [0.64, 0.44, 0.74]),
        (RockType::Andesite,       "Андезит",             [0.88, 0.32, 0.18]),
        (RockType::FloodBasalt,    "Траппы (базальт)",    [0.28, 0.25, 0.30]),
        (RockType::Sandstone,      "Песчаник",            [0.88, 0.76, 0.44]),
        (RockType::ShaleClay,      "Глины / сланцы",      [0.52, 0.62, 0.45]),
        (RockType::Limestone,      "Известняк",           [0.48, 0.84, 0.82]),
        (RockType::OceanicBasalt,  "Океанический базальт",[0.16, 0.22, 0.34]),
        (RockType::PelagicSediment,"Пелагические илы",    [0.26, 0.36, 0.48]),
    ];
    let _ = RockType::Granite; // тип держим для документации
    MapLegend {
        title: "Литология",
        entries: rocks.iter().map(|(_, label, rgb)| LegendEntry {
            rgb: rgb_u8(rgb[0], rgb[1], rgb[2]),
            label: (*label).into(),
        }).collect(),
    }
}

/// Легенда тектонических плит — показываем представителей каждого типа.
pub fn plates_legend() -> MapLegend {
    // Типичный цвет континентальной: HSL(50°, 0.75, 0.40) ≈ жёлто-оранжевый.
    // Типичный цвет океанической: HSL(220°, 0.55, 0.20) ≈ тёмно-синий.
    MapLegend {
        title: "Тектонические плиты",
        entries: vec![
            LegendEntry { rgb: rgb_u8(0.70, 0.62, 0.10), label: "Континентальная плита (красный -> жёлто-зелёный)".into() },
            LegendEntry { rgb: rgb_u8(0.09, 0.16, 0.31), label: "Океаническая плита (бирюзовый -> фиолетовый)".into() },
            LegendEntry { rgb: rgb_u8(0.10, 0.10, 0.10), label: "Вулкан".into() },
        ],
    }
}

/// Легенда рельефности — ключевые точки шкалы.
pub fn ruggedness_legend() -> MapLegend {
    MapLegend {
        title: "Рельефность",
        entries: vec![
            LegendEntry { rgb: rgb_u8(0.18, 0.56, 0.24), label: "Гладкие равнины  (0.00)".into() },
            LegendEntry { rgb: rgb_u8(0.70, 0.72, 0.18), label: "Холмы и овраги  (0.25)".into() },
            LegendEntry { rgb: rgb_u8(0.92, 0.30, 0.12), label: "Скалистые хребты  (0.60)".into() },
            LegendEntry { rgb: rgb_u8(0.87, 0.75, 0.90), label: "Пики и каньоны  (1.00)".into() },
        ],
    }
}