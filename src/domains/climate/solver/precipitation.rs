// src/domains/climate/solver/precipitation.rs

use rayon::prelude::*;

use super::{
    super::{climate_config::ClimateConfig, climate_data::SeasonalSnapshot},
    SeasonScratch, SolverPrecalc,
};
use crate::{domains::tectonics::tectonics_data::GeologyLayer, world::topology::WorldTopology};

#[allow(clippy::too_many_arguments)]
pub(super) fn compute_season_precipitation(
    topology: &WorldTopology,
    geology: &GeologyLayer,
    config: &ClimateConfig,
    precalc: &SolverPrecalc,
    w_air: &[f32],
    upwind_weight_sum: &[f32],
    wind_speed: &[f32],
    scratch: &mut SeasonScratch,
    snap: &mut SeasonalSnapshot,
) {
    let num_cells = topology.cells.len();
    let nbrs = &topology.cells;
    let elevs = &geology.elevations;
    let offs = &precalc.neighbor_offsets;
    let tdirs = &precalc.tangent_dirs_flat;
    let hex_rad = precalc.avg_hex_rad;

    // 1. Опорная скорость ветра и средний шаг для масштабонезависимого полета туч
    let sum_wind: f32 = wind_speed.iter().copied().sum();
    let avg_wind = (sum_wind / num_cells.max(1) as f32).max(1e-4);
    let ref_wind = avg_wind * 1.25;

    let sum_step_rate: f32 = wind_speed
        .iter()
        .map(|&sp| ((sp / ref_wind) * 0.72).clamp(0.06, 0.85))
        .sum();
    let mean_step_rate = (sum_step_rate / num_cells.max(1) as f32).max(0.10);

    let oro_strength = (config.orographic_rain_shadow / 7.0).clamp(0.0, 4.0);
    let precip_mult = config.precipitation_rate.max(0.0);
    let recycling = config.land_moisture_recycling.clamp(0.0, 0.85);

    let q_arr = &scratch.specific_humidity;
    let cap_arr = &scratch.moisture_capacity;
    let pressures = &snap.pressure;
    let winds_3d = &snap.winds_3d;

    // 2. Генерация туч (cloud_source) строго от ДИНАМИЧЕСКОГО ПОДЪЕМА (Правки A и D).
    // Используем scratch.prev_humidity как буфер источника туч cloud_source.
    let cloud_source = &mut scratch.prev_humidity;

    cloud_source
        .par_iter_mut()
        .enumerate()
        .for_each(|(i, src_out)| {
            let q = q_arr[i];
            let my_cap = cap_arr[i];
            let rh = (q / my_cap).clamp(0.0, 1.0);
            let my_p = pressures[i].clamp(-1.0, 1.0);
            let sp = wind_speed[i];
            let norm_sp = (sp / ref_wind).clamp(0.0, 1.40);
            let neighbors = &nbrs[i].neighbors;
            let start = offs[i];
            let end = offs[i + 1];

            // Запас абсолютной влаги в воздухе
            let avail_water = q.powf(0.65);

            // Правка D: жесткий нелинейный порог по RH (конденсация включается только при RH > 0.35
            // и растет по степени 1.5, разделяя "просто влажный воздух" и "насыщенные дождевые тучи")
            let rh_factor = ((rh - 0.35).max(0.0) / 0.65).powf(1.5);

            // --- МЕХАНИЗМ 1: Орографический и береговой подъем (upslope_lift) ---
            let w_sum = upwind_weight_sum[i];
            let mut upslope_lift = 0.0;
            let mut cooling_condensation = 0.0;

            if sp > 1e-5 && w_sum > 0.0 {
                let my_elev = elevs[i].max(0.0);
                let is_land = elevs[i] > 0.0;

                for j in start..end {
                    let w = w_air[j];
                    if w > 0.0 {
                        let n_id = neighbors[j - start];
                        let n_elev = elevs[n_id].max(0.0);
                        let dh = (my_elev - n_elev).max(0.0);

                        // Уклон вдоль ветра + легкий импульс при выходе морского ветра на берег
                        let coastal_step = if is_land && elevs[n_id] <= 0.0 { 0.12 } else { 0.0 };
                        let slope = (dh / hex_rad).clamp(0.0, 3.5) + coastal_step;
                        upslope_lift += slope * w;

                        // Фронтальное охлаждение (переход из теплой клетки в холодную)
                        let excess = (q_arr[n_id] - my_cap).max(0.0);
                        cooling_condensation += excess * w;
                    }
                }
                upslope_lift /= w_sum;
                cooling_condensation /= w_sum;
            }

            // Орографический подъем усиливается скоростью ветра и работает даже при умеренном RH (> 0.20)
            let oro_rh_factor = ((rh - 0.20).max(0.0) / 0.80).powf(1.2);
            let oro_clouds = (avail_water * oro_rh_factor * norm_sp * upslope_lift * 1.35
                + cooling_condensation * 1.10)
                * oro_strength;

            // --- МЕХАНИЗМ 2: Горизонтальная сходимость ветров (ВЗК, атмосферные фронты) ---
            let mut convergence = 0.0;
            let my_wind = winds_3d[i];
            for j in start..end {
                let n_id = neighbors[j - start];
                let inflow = (winds_3d[n_id] - my_wind).dot(-tdirs[j]);
                convergence += inflow;
            }
            if !neighbors.is_empty() {
                // Нормируем дивергенцию с учетом шага сетки hex_rad для масштабонезависимости
                let raw_conv = convergence / (neighbors.len() as f32 * avg_wind * hex_rad * 18.0);
                convergence = raw_conv.clamp(0.0, 2.0);
            }

            // --- МЕХАНИЗМ 3: Барический подъем (Правка A — БЕЗ положительного пола!) ---
            // В антициклонах (my_p >= 0.05) нисходящий воздух полностью выключает барический подъем (0.0)
            // и подавляет конвекцию через инверсию оседания!
            let cyclone_lift = (0.05 - my_p).max(0.0).powf(1.30) * 2.10;
            let inversion_suppression = (1.0 - my_p.max(0.0) * 1.85).clamp(0.0, 1.0);
            let conv_lift = convergence * 1.25 * inversion_suppression;

            let dynamic_lift = cyclone_lift + conv_lift;
            let surface_factor = if elevs[i] <= 0.0 { 0.75 } else { 1.0 };

            let syn_clouds = avail_water * rh_factor * dynamic_lift * 1.15 * surface_factor;

            *src_out = (oro_clouds + syn_clouds) * precip_mult;
        });

    snap.precipitation.copy_from_slice(cloud_source);

    // 3. ПОЛЕТ ТУЧ ПО ВЕТРУ (адвекция + накопление шлейфа за горами и берегами)
    let cloud_travel_rad = (0.75 * config.moisture_advection_strength).clamp(0.15, 1.80);
    let cloud_steps =
        ((cloud_travel_rad / (hex_rad * mean_step_rate)).round() as usize).clamp(25, 200);
    let inv_cloud_steps = 1.0 / cloud_steps as f32;

    let smooth_iters = ((0.07 / hex_rad).round() as usize).clamp(4, 18);
    let diff_fraction = (smooth_iters as f32 * inv_cloud_steps).clamp(0.02, 0.25);

    for _ in 0..cloud_steps {
        std::mem::swap(&mut snap.precipitation, &mut scratch.prev_precipitation);
        let prev_c = &scratch.prev_precipitation;
        let out_c = &mut snap.precipitation;

        out_c.par_iter_mut().enumerate().for_each(|(i, c_out)| {
            let sp = wind_speed[i];
            let neighbors = &nbrs[i].neighbors;
            let my_p = pressures[i].clamp(-1.0, 1.0);

            let norm_sp = (sp / ref_wind).clamp(0.0, 1.40);
            let step_rate = (norm_sp * 0.72).clamp(0.06, 0.85);
            let w_sum = upwind_weight_sum[i];

            // 1. Перенос туч ветром (Upwind)
            let incoming_c = if sp > 1e-5 && w_sum > 0.0 {
                let start = offs[i];
                let end = offs[i + 1];
                let mut upwind_c = 0.0;
                for j in start..end {
                    let w = w_air[j];
                    if w > 0.0 {
                        upwind_c += prev_c[neighbors[j - start]] * w;
                    }
                }
                upwind_c / w_sum
            } else {
                prev_c[i]
            };

            let mut c = prev_c[i] * (1.0 - step_rate) + incoming_c * step_rate;

            // 2. Турбулентное размытие облачного фронта
            if !neighbors.is_empty() {
                let chaos_mix = (0.48 - my_p * 0.22) * diff_fraction;
                let sum: f32 = neighbors.iter().map(|&n_id| prev_c[n_id]).sum();
                let nbr_c = sum / neighbors.len() as f32;
                c = c * (1.0 - chaos_mix) + nbr_c * chaos_mix;
            }

            // 3. Раздельная накачка (в горах/фронтах) и плавное затухание по пути следования туч:
            // Если в клетке есть мощный источник туч (cloud_source > c), туча быстро набирает силу.
            // Когда она сдувается ветром на равнину (где cloud_source мал), она тает медленно,
            // оставляя длинный дождевой шлейф по ветру (и быстрее испаряясь только в антициклонах).
            let src = cloud_source[i];
            if src > c {
                let build_rate = (5.5 * inv_cloud_steps).clamp(0.03, 0.45);
                c += (src - c) * build_rate;
            } else {
                let anticyclone_dry = if my_p > 0.0 { 1.0 + my_p * 2.2 } else { 0.85 };
                let decay_rate = (1.35 * inv_cloud_steps * anticyclone_dry).clamp(0.005, 0.20);
                c += (src - c) * decay_rate;
            }

            *c_out = c;
        });
    }

    // 4. Финальная нормализация карты осадков [0.0 .. 1.0] и минимальная связь с влажностью (Правка B)
    for i in 0..num_cells {
        let raw_p = snap.precipitation[i].max(0.0);
        let norm_precip = (1.0 - (-raw_p * 2.20).exp()).clamp(0.0, 1.0);
        snap.precipitation[i] = norm_precip;

        // Правка B: убираем сильное перетекание карты осадков в карту влажности (0.03 вместо 0.28)
        if elevs[i] > 0.0 {
            let rain_humidity_boost = norm_precip * 0.03 * recycling;
            snap.humidity[i] = (snap.humidity[i] + rain_humidity_boost).clamp(0.0, 1.0);
        }
    }
}