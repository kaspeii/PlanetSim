// src/domains/climate/solver/moisture.rs

use rayon::prelude::*;

use super::{
    super::{climate_config::ClimateConfig, climate_data::SeasonalSnapshot},
    SeasonScratch, SolverPrecalc, smoothstep,
};
use crate::{domains::tectonics::tectonics_data::GeologyLayer, world::topology::WorldTopology};

/// Максимальная влагоемкость воздуха C(T).
/// ВАЖНО: скобки вокруг всего выражения перед clamp и низкий порог 0.002,
/// чтобы даже при -50 °C емкость честно сжималась, поддерживая высокую RH на полюсах!
#[inline]
pub(super) fn saturation_capacity(temp_c: f32) -> f32 {
    (0.25 * (0.060 * temp_c).exp()).clamp(0.002, 12.0)
}

/// Доля открытой воды: 1.0 — чистая вода, ~0.08 — сплошной морской лёд.
#[inline]
fn open_water_fraction(albedo: f32, ocean_albedo: f32) -> f32 {
    let ice_factor = ((albedo - ocean_albedo) / 0.70).clamp(0.0, 1.0);
    (1.0 - 0.85 * ice_factor).max(0.08)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compute_season_moisture(
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
    let hex_rad = precalc.avg_hex_rad;

    // 1. Находим среднюю скорость ветра и реальное мат. ожидание E[step_rate] по сетке
    let sum_wind: f32 = wind_speed.iter().copied().sum();
    let avg_wind = (sum_wind / num_cells.max(1) as f32).max(1e-4);
    let ref_wind = avg_wind * 1.25;

    let sum_step_rate: f32 = wind_speed
        .iter()
        .map(|&sp| ((sp / ref_wind) * 0.72).clamp(0.05, 0.85))
        .sum();
    let mean_step_rate = (sum_step_rate / num_cells.max(1) as f32).max(0.10);

    // 2. Предрасчет потолка влагоемкости C(T) и стартового пара
    for i in 0..num_cells {
        let cap = saturation_capacity(snap.temperatures[i]);
        scratch.moisture_capacity[i] = cap;

        if elevs[i] <= 0.0 {
            let open_water = open_water_fraction(snap.albedo[i], config.ocean_albedo);
            // Надо льдом абсолютного пара мало (cap мал), но ОТНОСИТЕЛЬНАЯ влажность ~78..94%!
            let init_rh = 0.78 + 0.16 * open_water;
            scratch.specific_humidity[i] = cap * init_rh;
        } else {
            // На снежных/ледяных шапках суши воздух изначально имеет морозную сырость
            let snow_cover = ((snap.albedo[i] - config.land_albedo) / 0.50).clamp(0.0, 1.0);
            scratch.specific_humidity[i] = cap * (0.15 + 0.55 * snow_cover);
        }
    }

    // --- ФИЗИЧЕСКИЕ МАСШТАБЫ В РАДИАНАХ СФЕРЫ ---
    let total_advection_rad = (1.40 * config.moisture_advection_strength).clamp(0.20, 3.00);
    let num_steps =
        ((total_advection_rad / (hex_rad * mean_step_rate)).round() as usize).clamp(25, 350);

    // Вычисляем долю диффузии на каждый шаг адвекции.
    // Раньше было smooth_iters циклов, теперь размазываем их на num_steps.
    let smooth_iters = ((0.04 / hex_rad).round() as usize).clamp(2, 10);
    let diff_fraction = (smooth_iters as f32 / num_steps as f32).clamp(0.01, 0.20);

    for _ in 0..num_steps {
        std::mem::swap(&mut scratch.specific_humidity, &mut scratch.prev_humidity);
        let prev = &scratch.prev_humidity;
        let cap_arr = &scratch.moisture_capacity;
        let out = &mut scratch.specific_humidity;

        out.par_iter_mut().enumerate().for_each(|(i, q_out)| {
            // --- Океан — стабильное граничное условие ---
            if elevs[i] <= 0.0 {
                *q_out = prev[i];
                return;
            }

            let sp = wind_speed[i];
            let my_cap = cap_arr[i];
            let neighbors = &nbrs[i].neighbors;
            let my_p = snap.pressure[i].clamp(-1.0, 1.0);

            let norm_sp = sp / ref_wind;
            let step_rate = norm_sp * 0.72;
            let w_sum = upwind_weight_sum[i];

            // 1. Чистый Upwind (Ветер)
            let incoming_q = if sp > 1e-5 && w_sum > 0.0 {
                let start = offs[i];
                let end = offs[i + 1];
                let mut upwind_q = 0.0;
                for j in start..end {
                    let w = w_air[j];
                    if w > 0.0 {
                        upwind_q += prev[neighbors[j - start]] * w;
                    }
                }
                upwind_q / w_sum
            } else {
                prev[i]
            };

            let mut q = prev[i] * (1.0 - step_rate) + incoming_q * step_rate;

            // 2. Турбулентная диффузия (Вихри)
            // Применяется на каждом шаге с малой силой, размывая прямые линии
            if !neighbors.is_empty() {
                let chaos_mix = (0.45 - my_p * 0.20) * diff_fraction;
                let sum: f32 = neighbors.iter().map(|&n_id| prev[n_id]).sum();
                let nbr_q = sum / neighbors.len() as f32;
                q = q * (1.0 - chaos_mix) + nbr_q * chaos_mix;
            }

            // 3. Субсиденция (Иссушение в антициклонах)
            // Теперь она происходит во время движения воздуха, а не в конце
            if my_p > 0.0 {
                let dry_target = my_cap * 0.15;
                let cold_factor = smoothstep((5.0 - snap.temperatures[i]) / 25.0);
                let subsidence_scale = 0.45 * (1.0 - 0.60 * cold_factor);
                
                // Переводим разовую скорость в скорость "за шаг" (экспоненциальное сглаживание)
                let one_shot_rate = (my_p * subsidence_scale * config.subsidence_drying_strength).clamp(0.0, 1.0);
                let step_sub_rate = 1.0 - (1.0 - one_shot_rate).powf(1.0 / num_steps as f32);

                if q > dry_target {
                    q += (dry_target - q) * step_sub_rate;
                }
            }

            // 4. Естественная конденсация от температуры (Горы)
            *q_out = q.min(my_cap);
        });
    }

    // Финальный расчет относительной влажности
    for i in 0..num_cells {
        snap.humidity[i] = (scratch.specific_humidity[i] / scratch.moisture_capacity[i]).clamp(0.0, 1.0);
    }
}