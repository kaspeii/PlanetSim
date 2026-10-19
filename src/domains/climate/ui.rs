use super::{
    config::ClimateConfig,
    data::{ClimateLayer, ClimateSeason},
};
use crate::{
    domains::tectonics::data::GeologyLayer,
    viewport::Globe,
    world::{WorldConfig, pipeline::WorldPipeline, topology::WorldTopology},
};
use bevy::prelude::*;
use bevy_egui::egui;
use std::collections::HashMap;

pub fn build_temperature_colors(climate: &ClimateLayer) -> Vec<[f32; 4]> {
    climate
        .temperatures
        .iter()
        .map(|&t| temperature_to_color(t).to_linear().to_f32_array())
        .collect()
}

fn temperature_to_color(t: f32) -> Color {
    // Узловые точки: (Температура °C, R, G, B)
    const STOPS: &[(f32, [f32; 3])] = &[
        (-30.0, [0.92, 0.94, 1.00]), // Белый / Арктический лед
        (-15.0, [0.60, 0.20, 0.80]), // Фиолетовый (суровый мороз)
        (0.0, [0.05, 0.35, 0.85]),   // Синий (точка замерзания)
        (12.0, [0.20, 0.72, 0.40]),  // Зеленый (умеренный климат)
        (22.0, [0.92, 0.82, 0.20]),  // Желтый (субтропики)
        (30.0, [0.92, 0.40, 0.10]),  // Оранжевый (жара)
        (42.0, [0.65, 0.08, 0.08]),  // Темно-красный (пустынный зной)
    ];

    if t <= STOPS[0].0 {
        let [r, g, b] = STOPS[0].1;
        return Color::srgb(r, g, b);
    }
    for window in STOPS.windows(2) {
        let (t0, c0) = window[0];
        let (t1, c1) = window[1];
        if t <= t1 {
            let f = (t - t0) / (t1 - t0);
            return Color::srgb(
                c0[0] + (c1[0] - c0[0]) * f,
                c0[1] + (c1[1] - c0[1]) * f,
                c0[2] + (c1[2] - c0[2]) * f,
            );
        }
    }
    let [r, g, b] = STOPS.last().unwrap().1;
    Color::srgb(r, g, b)
}

pub fn build_albedo_colors(climate: &ClimateLayer) -> Vec<[f32; 4]> {
    climate
        .albedo
        .iter()
        .map(|&a| Color::srgb(a, a, a).to_linear().to_f32_array())
        .collect()
}

pub fn build_continentality_colors(climate: &ClimateLayer) -> Vec<[f32; 4]> {
    climate
        .continentality
        .iter()
        .map(|&c| {
            let color = if c < 0.0 {
                // ОКЕАН: c ∈ [-1.0 (открытый океан) .. 0.0 (берег/маргинальное море)]
                // t = 0.0 у берега, t = 1.0 далеко в открытом океане
                let t = (-c).clamp(0.0, 1.0);
                // От светло-бирюзового у берегов (0.45, 0.78, 0.88) к глубокому синему (0.05, 0.15, 0.45)
                Color::srgb(0.45 - 0.40 * t, 0.78 - 0.63 * t, 0.88 - 0.43 * t)
            } else {
                // СУША: c ∈ [0.0 (побережье) .. +1.0 (глубь материка)]
                let t = c.clamp(0.0, 1.0);
                // Двухступенчатый градиент суши:
                // 0.0 .. 0.5: Светло-бежевый (0.92, 0.85, 0.68) -> Золотистая охра (0.85, 0.55, 0.30)
                // 0.5 .. 1.0: Золотистая охра (0.85, 0.55, 0.30) -> Глубокий терракотово-бурый (0.48, 0.15, 0.10)
                if t < 0.5 {
                    let s = t * 2.0;
                    Color::srgb(0.92 - 0.07 * s, 0.85 - 0.30 * s, 0.68 - 0.38 * s)
                } else {
                    let s = (t - 0.5) * 2.0;
                    Color::srgb(0.85 - 0.37 * s, 0.55 - 0.40 * s, 0.30 - 0.20 * s)
                }
            };
            color.to_linear().to_f32_array()
        })
        .collect()
}

/// Палитра атмосферного давления: Синий = Низкое (Циклоны/ITCZ), Белый = Норма, Красный = Высокое (Антициклоны)
pub fn build_pressure_colors(climate: &ClimateLayer) -> Vec<[f32; 4]> {
    climate
        .pressure
        .iter()
        .map(|&p| {
            let norm = (p).clamp(-1.0, 1.0);
            let color = if norm < 0.0 {
                // От синего (-1.0) к белому (0.0)
                let t = norm + 1.0;
                Color::srgb(0.1 + 0.85 * t, 0.25 + 0.7 * t, 0.95)
            } else {
                // От белого (0.0) к красно-оранжевому (+1.0)
                let t = norm;
                Color::srgb(0.95, 0.95 - 0.75 * t, 0.95 - 0.85 * t)
            };
            color.to_linear().to_f32_array()
        })
        .collect()
}

/// Палитра силы ветра (как на метеокартах): Синий (штиль) -> Бирюзовый -> Желтый -> Красный (шторм)
pub fn build_wind_colors(climate: &ClimateLayer) -> Vec<[f32; 4]> {
    let max_speed = climate
        .winds
        .iter()
        .map(|w| w.length())
        .fold(0.0001_f32, f32::max);

    climate
        .winds
        .iter()
        .map(|&w| {
            let s = (w.length() / max_speed).clamp(0.0, 1.0).sqrt();
            wind_speed_to_color(s).to_linear().to_f32_array()
        })
        .collect()
}

/// Цвет ветра по нормированной силе [0..1]. Общий для меша и 3D-стрелок.
fn wind_speed_to_color(s: f32) -> Color {
    let s = s.clamp(0.0, 1.0);
    match s {
        x if x < 0.25 => {
            let t = x / 0.25;
            Color::srgb(0.08, 0.12 + 0.48 * t, 0.35 + 0.45 * t) // тёмно-синий → голубой
        }
        x if x < 0.50 => {
            let t = (x - 0.25) / 0.25;
            Color::srgb(0.08 + 0.22 * t, 0.60 + 0.25 * t, 0.80 - 0.40 * t) // голубой → зелёный
        }
        x if x < 0.75 => {
            let t = (x - 0.50) / 0.25;
            Color::srgb(0.30 + 0.65 * t, 0.85, 0.40 - 0.30 * t) // зелёный → жёлтый
        }
        _ => {
            let t = (s - 0.75) / 0.25;
            Color::srgb(0.95, 0.85 - 0.70 * t, 0.10 + 0.30 * t) // жёлтый → красный
        }
    }
}

/// Кэш отфильтрованных стрелок, чтобы не пересчитывать HashMap каждый кадр!
#[derive(Default)]
pub struct WindArrowsCache {
    pub cached_season: Option<ClimateSeason>,
    /// Хранит (индекс клетки, нормированная скорость s в 0..1)
    pub arrows: Vec<(usize, f32)>,
}

/// Отрисовка 3D-стрелок ветра поверх глобуса (БЕЗ пересчета сетки каждый кадр!)
pub fn draw_wind_arrows_gizmos(
    mut gizmos: Gizmos,
    mut cache: Local<WindArrowsCache>,
    topology: Res<WorldTopology>,
    geology: Res<GeologyLayer>,
    climate: Res<ClimateLayer>,
    config: Res<ClimateConfig>,
    world_config: Res<WorldConfig>,
    globe_query: Query<&Transform, With<Globe>>,
) {
    if !config.show_wind_arrows || climate.winds_3d.is_empty() || geology.elevations.is_empty() {
        return;
    }
    let Ok(globe_transform) = globe_query.single() else {
        return;
    };

    // Пересчитываем сетку стрелок ТОЛЬКО если изменился климат, сезон или размер планеты!
    if climate.is_changed()
        || config.is_changed()
        || cache.cached_season != Some(config.active_season)
    {
        cache.cached_season = Some(config.active_season);
        cache.arrows.clear();

        let max_speed = climate
            .winds_3d
            .iter()
            .map(|w| w.length())
            .fold(0.0001_f32, f32::max);

        // Сетка 12x12x12 дает ~600–800 хорошо читаемых стрелок на сферу
        let grid_size = 12_i32;
        let inv_cell_size = grid_size as f32 * 0.5;

        let mut best_per_cell: HashMap<(i32, i32, i32), (usize, f32)> =
            HashMap::with_capacity(1024);

        for (i, w) in climate.winds_3d.iter().enumerate() {
            let speed = w.length();
            if speed < max_speed * 0.05 {
                continue;
            }
            let pos = topology.cells[i].center;
            let key = (
                ((pos.x + 1.0) * inv_cell_size) as i32,
                ((pos.y + 1.0) * inv_cell_size) as i32,
                ((pos.z + 1.0) * inv_cell_size) as i32,
            );
            let entry = best_per_cell.entry(key).or_insert((i, speed));
            if speed > entry.1 {
                *entry = (i, speed);
            }
        }

        for &(i, speed) in best_per_cell.values() {
            let s = (speed / max_speed).clamp(0.0, 1.0).sqrt();
            cache.arrows.push((i, s));
        }
    }

    // Масштабируем длину стрелки от радиуса планеты, чтобы их всегда было отлично видно!
    let r = world_config.radius;
    let base_len = r * 0.08; // 8% от радиуса планеты для максимального ветра

    // Каждый кадр просто рисуем уже готовые стрелки из кэша (работает мгновенно!)
    for &(i, s) in &cache.arrows {
        let wind_dir = climate.winds_3d[i].normalize_or_zero();
        let cell_dir = topology.cells[i].center;
        let h = geology.elevations[i].max(0.0);

        let local_start = cell_dir * (r + h + r * 0.015);
        let arrow_len = base_len * (0.35 + 0.65 * s);
        let local_end = local_start + wind_dir * arrow_len;

        let world_start = globe_transform.transform_point(local_start);
        let world_end = globe_transform.transform_point(local_end);

        let color = wind_speed_to_color(s).with_alpha(0.45 + 0.55 * s);
        gizmos.arrow(world_start, world_end, color);
    }
}

#[derive(Default)]
pub struct OceanCurrentsCache {
    pub cached_season: Option<ClimateSeason>,
    pub arrows: Vec<(usize, f32)>,
}

pub fn draw_ocean_currents_gizmos(
    mut gizmos: Gizmos,
    mut cache: Local<OceanCurrentsCache>,
    topology: Res<WorldTopology>,
    geology: Res<GeologyLayer>,
    climate: Res<ClimateLayer>,
    config: Res<ClimateConfig>,
    world_config: Res<WorldConfig>,
    globe_query: Query<&Transform, With<Globe>>,
) {
    if !config.show_ocean_currents
        || climate.ocean_currents.is_empty()
        || geology.elevations.is_empty()
    {
        return;
    }
    let Ok(globe_transform) = globe_query.single() else {
        return;
    };

    if climate.is_changed()
        || config.is_changed()
        || cache.cached_season != Some(config.active_season)
    {
        cache.cached_season = Some(config.active_season);
        cache.arrows.clear();

        let max_speed = climate
            .ocean_currents
            .iter()
            .map(|c| c.length())
            .fold(0.0001_f32, f32::max);

        let grid_size = 14_i32; // Чуть плотнее сетка, чтобы показать струи у берегов
        let inv_cell_size = grid_size as f32 * 0.5;
        let mut best_per_cell: HashMap<(i32, i32, i32), (usize, f32)> =
            HashMap::with_capacity(1024);

        for (i, current) in climate.ocean_currents.iter().enumerate() {
            let speed = current.length();
            if speed < max_speed * 0.05 {
                continue;
            }

            let pos = topology.cells[i].center;
            let key = (
                ((pos.x + 1.0) * inv_cell_size) as i32,
                ((pos.y + 1.0) * inv_cell_size) as i32,
                ((pos.z + 1.0) * inv_cell_size) as i32,
            );

            let entry = best_per_cell.entry(key).or_insert((i, speed));
            if speed > entry.1 {
                *entry = (i, speed);
            }
        }

        for &(i, speed) in best_per_cell.values() {
            let s = (speed / max_speed).clamp(0.0, 1.0).sqrt();
            cache.arrows.push((i, s));
        }
    }

    let r = world_config.radius;
    let base_len = r * 0.07;

    for &(i, s) in &cache.arrows {
        let current_dir = climate.ocean_currents[i].normalize_or_zero();
        let cell_dir = topology.cells[i].center;

        // Течения рисуем прямо на уровне моря
        let local_start = cell_dir * (r + r * 0.005);
        let arrow_len = base_len * (0.35 + 0.65 * s);
        let local_end = local_start + current_dir * arrow_len;

        let world_start = globe_transform.transform_point(local_start);
        let world_end = globe_transform.transform_point(local_end);

        // Цвет течений от темно-синего до ярко-голубого и бирюзового
        let color =
            Color::srgb(0.05 + 0.1 * s, 0.3 + 0.6 * s, 0.8 + 0.2 * s).with_alpha(0.6 + 0.4 * s);
        gizmos.arrow(world_start, world_end, color);
    }
}

/// Правая панель настроек климата для bevy_egui
pub fn draw_climate_tab(
    ui: &mut egui::Ui,
    config: &mut ClimateConfig,
    climate: &mut ClimateLayer,
    pipeline: &mut WorldPipeline,
) {
    ui.heading("Климат и Атмосфера");
    ui.separator();

    // --- БЫСТРЫЙ ПЕРЕКЛЮЧАТЕЛЬ ПРЕДРАССЧИТАННЫХ СЕЗОНОВ ---
    ui.label("Отображаемый сезон (Сев. полушарие):");
    let mut season_changed = false;
    ui.horizontal(|ui| {
        season_changed |= ui
            .selectable_value(
                &mut config.active_season,
                ClimateSeason::Spring,
                "Весна / Осень",
            )
            .changed();
        season_changed |= ui
            .selectable_value(&mut config.active_season, ClimateSeason::Summer, "Лето")
            .changed();
        season_changed |= ui
            .selectable_value(&mut config.active_season, ClimateSeason::Winter, "Зима")
            .changed();
    });

    if season_changed {
        // Не пересчитываем физику! Просто подставляем уже готовый снимок сезона:
        climate.apply_active_season(config.active_season);
        pipeline.recolor_mesh = true;
    }

    ui.separator();
    let mut sim_changed = false;

    ui.label("Температура и Инсоляция");
    sim_changed |= ui
        .add(egui::Slider::new(&mut config.equator_temp, 0.0..=60.0).text("Экватор (°C)"))
        .changed();
    sim_changed |= ui
        .add(egui::Slider::new(&mut config.pole_temp, -70.0..=10.0).text("Полюса (°C)"))
        .changed();
    sim_changed |= ui
        .add(
            egui::Slider::new(&mut config.altitude_lapse_rate, 0.0..=100.0)
                .text("Остывание в горах"),
        )
        .changed();
    sim_changed |= ui
        .add(
            egui::Slider::new(&mut config.freezing_temp, -25.0..=5.0).text("Порог замерзания льда"),
        )
        .changed();
    sim_changed |= ui
        .add(egui::Slider::new(&mut config.axial_tilt, 0.0..=0.8).text("Наклон оси (рад)"))
        .changed();

    ui.separator();
    ui.label("Континентальность");
    sim_changed |= ui
        .add(egui::Slider::new(&mut config.base_ocean_moderation, 0.0..=1.0).text("Инерция океана"))
        .changed();
    sim_changed |= ui
        .add(
            egui::Slider::new(&mut config.land_continentality_scale, 0.1..=1.5)
                .text("Размер береговой зоны (рад)"),
        )
        .changed();
    sim_changed |= ui
        .add(
            egui::Slider::new(&mut config.ocean_continentality_scale, 0.02..=0.6)
                .text("Зона окраинных морей (рад)"),
        )
        .changed();

    ui.separator();
    ui.label("Давление и Ветра");
    sim_changed |= ui
        .add(
            egui::Slider::new(&mut config.latitudinal_pressure_weight, 0.0..=2.0)
                .text("Сила поясов (Хэдли и др.)"),
        )
        .changed();
    sim_changed |= ui
        .add(
            egui::Slider::new(&mut config.thermal_pressure_weight, 0.0..=2.0)
                .text("Влияние материков"),
        )
        .changed();
    sim_changed |= ui
        .add(
            egui::Slider::new(&mut config.coriolis_strength, 0.0..=1.57)
                .text("Сила Кориолиса (рад)"),
        )
        .changed();
    sim_changed |= ui
        .add(egui::Slider::new(&mut config.mountain_wind_barrier, 0.0..=25.0).text("Барьер гор"))
        .changed();
    sim_changed |= ui
        .add(
            egui::Slider::new(&mut config.wind_speed_multiplier, 1.0..=30.0).text("Скорость ветра"),
        )
        .changed();
    sim_changed |= ui
        .add(
            egui::Slider::new(&mut config.heat_advection_strength, 0.0..=1.5)
                .text("Распространение температуры"),
        )
        .changed();
    sim_changed |= ui
        .add(
            egui::Slider::new(&mut config.equilibrium_iterations, 1..=8)
                .text("Итерации давления-температуры"),
        )
        .changed();

    ui.separator();
    ui.checkbox(&mut config.show_wind_arrows, "Показать 3D-стрелки ветров");
    if ui
        .checkbox(
            &mut config.show_ocean_currents,
            "Показать 3D-стрелки течений",
        )
        .changed()
    {
        pipeline.rebuild_mesh = true;
    }

    if sim_changed {
        pipeline.regenerate_climate = true;
    }
}
