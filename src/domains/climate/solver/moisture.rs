// src/domains/climate/solver/moisture.rs

use rayon::prelude::*;

use super::{
    super::{climate_config::ClimateConfig, climate_data::SeasonalSnapshot},
    SeasonScratch, SolverPrecalc, smoothstep,
};
use crate::{domains::tectonics::tectonics_data::GeologyLayer, world::topology::WorldTopology};

/// Максимальная влагоемкость воздуха C(T)[cite: 1].
#[inline]
pub(super) fn saturation_capacity(temp_c: f32) -> f32 {
    (0.25 * (0.060 * temp_c).exp()).clamp(0.002, 12.0)
}

/// Доля открытой воды: 1.0 — чистая вода, ~0.08 — сплошной морской лёд[cite: 1].
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

    // 1. Средняя скорость ветра и мат. ожидание E[step_rate] по сетке[cite: 1]
    let sum_wind: f32 = wind_speed.iter().copied().sum();
    let avg_wind = (sum_wind / num_cells.max(1) as f32).max(1e-4);
    let ref_wind = avg_wind * 1.25;

    let sum_step_rate: f32 = wind_speed
        .iter()
        .map(|&sp| ((sp / ref_wind) * 0.72).clamp(0.05, 0.85))
        .sum();
    let mean_step_rate = (sum_step_rate / num_cells.max(1) as f32).max(0.10);

    // 2. Предрасчет потолка влагоемкости C(T) и стартового пара (Правка C: учет давления над океаном)
    for i in 0..num_cells {
        let cap = saturation_capacity(snap.temperatures[i]);
        scratch.moisture_capacity[i] = cap;

        if elevs[i] <= 0.0 {
            let open_water = open_water_fraction(snap.albedo[i], config.ocean_albedo);
            let base_rh = 0.76 + 0.18 * open_water;
            // Субтропические максимумы над океаном суше из-за инверсии оседания
            let subsidence_mod = 1.0 - 0.32 * snap.pressure[i].clamp(-1.0, 1.0).max(0.0);
            let init_rh =
                (base_rh * subsidence_mod) * config.ocean_evaporation_rate.clamp(0.5, 1.15);
            scratch.specific_humidity[i] = cap * init_rh.clamp(0.15, 0.96);
        } else {
            let snow_cover = ((snap.albedo[i] - config.land_albedo) / 0.50).clamp(0.0, 1.0);
            scratch.specific_humidity[i] = cap * (0.12 + 0.50 * snow_cover);
        }
    }

    // --- ФИЗИЧЕСКИЕ МАСШТАБЫ В РАДИАНАХ СФЕРЫ ---
    let total_advection_rad = (1.40 * config.moisture_advection_strength).clamp(0.20, 3.00);
    let num_steps =
        ((total_advection_rad / (hex_rad * mean_step_rate)).round() as usize).clamp(25, 350);
    let inv_steps = 1.0 / num_steps as f32;

    let smooth_iters = ((0.04 / hex_rad).round() as usize).clamp(2, 10);
    let diff_fraction = (smooth_iters as f32 * inv_steps).clamp(0.01, 0.20);

    // Расход пара над сушей (рециклинг влаги лесами/почвой работает здесь, а не через приплюсовывание осадков к RH)
    let net_land_loss =
        (0.28 * (1.0 - config.land_moisture_recycling.clamp(0.0, 0.85)) * inv_steps)
            .clamp(0.0, 0.05);
    let mountain_shadow_rate = (config.orographic_rain_shadow / 12.0).clamp(0.05, 1.0);

    for _ in 0..num_steps {
        std::mem::swap(&mut scratch.specific_humidity, &mut scratch.prev_humidity);
        let prev = &scratch.prev_humidity;
        let cap_arr = &scratch.moisture_capacity;
        let out = &mut scratch.specific_humidity;

        out.par_iter_mut().enumerate().for_each(|(i, q_out)| {
            let is_ocean = elevs[i] <= 0.0;
            let sp = wind_speed[i];
            let my_cap = cap_arr[i];
            let neighbors = &nbrs[i].neighbors;
            let my_p = snap.pressure[i].clamp(-1.0, 1.0);

            let norm_sp = (sp / ref_wind).clamp(0.0, 1.35);
            let step_rate = (norm_sp * 0.72).clamp(0.05, 0.85);
            let w_sum = upwind_weight_sum[i];

            // 1. Чистый Upwind (теперь работает И над сушей, И над океаном!)
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

            // 2. Турбулентная диффузия (Вихри)[cite: 1]
            if !neighbors.is_empty() {
                let chaos_mix = (0.45 - my_p * 0.20) * diff_fraction;
                let sum: f32 = neighbors.iter().map(|&n_id| prev[n_id]).sum();
                let nbr_q = sum / neighbors.len() as f32;
                q = q * (1.0 - chaos_mix) + nbr_q * chaos_mix;
            }

            if is_ocean {
                // Океан: непрерывная подпитка испарением, но ограниченная инверсией в антициклонах (Правка C)
                let open_water = open_water_fraction(snap.albedo[i], config.ocean_albedo);
                let base_rh = 0.78 + 0.16 * open_water;
                let subsidence_mod = 1.0 - 0.35 * (my_p.max(0.0) * config.subsidence_drying_strength).clamp(0.0, 1.0);
                let target_rh = (base_rh * subsidence_mod * config.ocean_evaporation_rate.clamp(0.5, 1.15))
                    .clamp(0.20, 0.96);
                let target_q = my_cap * target_rh;

                // Скорость релаксации над океаном: позволяет сухим континентальным ветрам
                // постепенно набираться влаги над прибрежным морем, а антициклонам — подсушивать океан
                let evap_step = (4.5 * open_water * config.ocean_evaporation_rate * inv_steps).clamp(0.02, 0.35);
                q += (target_q - q) * evap_step;
            } else {
                // Суша: расход влаги вглубь континента + субсиденция в антициклонах[cite: 1]
                q *= 1.0 - net_land_loss;

                if my_p > 0.0 {
                    let dry_target = my_cap * 0.15;
                    let cold_factor = smoothstep((5.0 - snap.temperatures[i]) / 25.0);
                    let subsidence_scale = 0.45 * (1.0 - 0.60 * cold_factor);

                    let one_shot_rate =
                        (my_p * subsidence_scale * config.subsidence_drying_strength).clamp(0.0, 1.0);
                    let step_sub_rate = 1.0 - (1.0 - one_shot_rate).powf(inv_steps);

                    if q > dry_target {
                        q += (dry_target - q) * step_sub_rate;
                    }
                }
            }

            // 4. Конденсация на холодных хребтах (формирует дождевую тень за горами)
            if q > my_cap {
                q -= (q - my_cap) * mountain_shadow_rate;
            }
            *q_out = q.min(my_cap);
        });
    }

    // Финальный расчет относительной влажности[cite: 1]
    for i in 0..num_cells {
        snap.humidity[i] =
            (scratch.specific_humidity[i] / scratch.moisture_capacity[i]).clamp(0.0, 1.0);
    }
}