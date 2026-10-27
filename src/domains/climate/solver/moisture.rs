// src/domains/climate/solver/moisture.rs

use rayon::prelude::*;

use super::{
    super::{climate_config::ClimateConfig, climate_data::SeasonalSnapshot},
    SeasonScratch, SolverPrecalc, smoothstep,
};
use crate::{domains::tectonics::tectonics_data::GeologyLayer, world::topology::WorldTopology};

#[inline]
pub(super) fn saturation_capacity(temp_c: f32) -> f32 {
    (0.25 * (0.060 * temp_c).exp()).clamp(0.002, 12.0)
}

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
    surface_albedo: &[f32],
    surface_wetness: &[f32],
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

    let sum_wind: f32 = wind_speed.iter().copied().sum();
    let avg_wind = (sum_wind / num_cells.max(1) as f32).max(1e-4);
    let ref_wind = avg_wind * 1.25;

    let sum_step_rate: f32 = wind_speed
        .iter()
        .map(|&sp| ((sp / ref_wind) * 0.72).clamp(0.05, 0.85))
        .sum();
    let mean_step_rate = (sum_step_rate / num_cells.max(1) as f32).max(0.10);

    // Стартовый пар с учетом влажности поверхности (рек, почв, лесов) и снежного покрова
    for i in 0..num_cells {
        let cap = saturation_capacity(snap.temperatures[i]);
        scratch.moisture_capacity[i] = cap;

        if elevs[i] <= 0.0 {
            let open_water = open_water_fraction(snap.albedo[i], config.ocean_albedo);
            let base_rh = 0.76 + 0.18 * open_water;
            let subsidence_mod = 1.0 - 0.32 * snap.pressure[i].clamp(-1.0, 1.0).max(0.0);
            let init_rh =
                (base_rh * subsidence_mod) * config.ocean_evaporation_rate.clamp(0.5, 1.15);
            scratch.specific_humidity[i] = cap * init_rh.clamp(0.15, 0.96);
        } else {
            let base_alb = surface_albedo[i];
            let wetness = surface_wetness[i];
            let snow_cover = ((snap.albedo[i] - base_alb) / 0.50).clamp(0.0, 1.0);
            let init_land_rh = (0.06 + 0.36 * wetness + 0.45 * snow_cover).clamp(0.05, 0.88);
            scratch.specific_humidity[i] = cap * init_land_rh;
        }
    }

    let total_advection_rad = (1.40 * config.moisture_advection_strength).clamp(0.20, 3.00);
    let num_steps =
        ((total_advection_rad / (hex_rad * mean_step_rate)).round() as usize).clamp(25, 350);
    let inv_steps = 1.0 / num_steps as f32;

    let smooth_iters = ((0.04 / hex_rad).round() as usize).clamp(2, 10);
    let diff_fraction = (smooth_iters as f32 * inv_steps).clamp(0.01, 0.20);

    let recycling_cfg = config.land_moisture_recycling.clamp(0.0, 0.95);
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

            if !neighbors.is_empty() {
                let chaos_mix = (0.45 - my_p * 0.20) * diff_fraction;
                let sum: f32 = neighbors.iter().map(|&n_id| prev[n_id]).sum();
                let nbr_q = sum / neighbors.len() as f32;
                q = q * (1.0 - chaos_mix) + nbr_q * chaos_mix;
            }

            if is_ocean {
                let open_water = open_water_fraction(snap.albedo[i], config.ocean_albedo);
                let base_rh = 0.78 + 0.16 * open_water;
                let subsidence_mod =
                    1.0 - 0.35 * (my_p.max(0.0) * config.subsidence_drying_strength).clamp(0.0, 1.0);
                let target_rh = (base_rh
                    * subsidence_mod
                    * config.ocean_evaporation_rate.clamp(0.5, 1.15))
                .clamp(0.20, 0.96);
                let target_q = my_cap * target_rh;

                let evap_step =
                    (4.5 * open_water * config.ocean_evaporation_rate * inv_steps).clamp(0.02, 0.35);
                q += (target_q - q) * evap_step;
            } else {
                // ОБРАТНАЯ СВЯЗЬ СУШИ:
                // 1. Локальное удержание влаги зависит от реальной влажности почвы, биома и рек
                let wetness = surface_wetness[i];
                let local_recycling = (recycling_cfg * (0.25 + 1.25 * wetness)).clamp(0.0, 0.94);
                let local_land_loss = (0.32 * (1.0 - local_recycling) * inv_steps).clamp(0.0, 0.06);
                q *= 1.0 - local_land_loss;

                // 2. Активное испарение с рек, болот и транспирация лесов (биотический насос):
                // Крупная река или влажный лес дозаряжают проходящий воздух водяным паром!
                if wetness > 0.12 && snap.temperatures[i] > -4.0 {
                    let evap_target_rh = (0.22 + 0.64 * wetness).clamp(0.18, 0.88);
                    let evap_target_q = my_cap * evap_target_rh;
                    if q < evap_target_q {
                        let evap_rate =
                            (2.4 * wetness * recycling_cfg * inv_steps).clamp(0.0, 0.18);
                        q += (evap_target_q - q) * evap_rate;
                    }
                }

                // 3. Оседание воздуха в антициклонах
                if my_p > 0.0 {
                    let dry_target = my_cap * (0.12 + 0.15 * wetness);
                    let cold_factor = smoothstep((5.0 - snap.temperatures[i]) / 25.0);
                    let subsidence_scale = 0.45 * (1.0 - 0.60 * cold_factor);

                    let one_shot_rate = (my_p * subsidence_scale * config.subsidence_drying_strength)
                        .clamp(0.0, 1.0);
                    let step_sub_rate = 1.0 - (1.0 - one_shot_rate).powf(inv_steps);

                    if q > dry_target {
                        q += (dry_target - q) * step_sub_rate;
                    }
                }
            }

            if q > my_cap {
                q -= (q - my_cap) * mountain_shadow_rate;
            }
            *q_out = q.min(my_cap);
        });
    }

    for i in 0..num_cells {
        snap.humidity[i] =
            (scratch.specific_humidity[i] / scratch.moisture_capacity[i]).clamp(0.0, 1.0);
    }
}
