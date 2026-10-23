// src/domains/climate/solver/moisture.rs

use rayon::prelude::*;

use super::{
    super::{climate_config::ClimateConfig, climate_data::SeasonalSnapshot},
    SeasonScratch, SolverPrecalc,
};
use crate::{domains::tectonics::tectonics_data::GeologyLayer, world::topology::WorldTopology};

/// Перевод абстрактной высоты в физическую для орографической поправки.
const OROGRAPHIC_VERTICAL_SCALE: f32 = 0.02;

/// Максимальная влагоемкость воздуха.
///
/// Экспоненциальная аппроксимация зависимости насыщения от температуры.
/// Это НЕ строгий Клаузиус–Клапейрон: для него нужна exp(-L/Rv · 1/T),
/// т.е. экспонента от ОБРАТНОЙ температуры. Здесь влагоемкость растёт
/// примерно вдвое на каждые ~15 °C — этого достаточно для симуляции.
#[inline]
pub(super) fn saturation_capacity(temp_c: f32) -> f32 {
    0.25 * (0.045 * temp_c).exp().clamp(0.06, 10.0)
}

/// Доля открытой воды: 1.0 — чистая вода, ~0.04 — почти сплошной лёд.
#[inline]
fn open_water_fraction(albedo: f32, ocean_albedo: f32) -> f32 {
    let ice_factor = ((albedo - ocean_albedo) / 0.70).clamp(0.0, 1.0);
    (1.0 - 0.85 * ice_factor).max(0.04)
}

/// Множитель осушения при подъёме с `from_elev` на `to_elev`.
///
/// `exp(-dh * k)` для положительного перепада над `mountain_base`,
/// иначе `1.0`. Единая формула для адвекции и сглаживания, чтобы
/// орографический эффект не применялся двумя разными способами дважды.
#[inline]
fn orographic_factor(
    from_elev: f32,
    to_elev: f32,
    mountain_base: f32,
    k: f32,
) -> f32 {
    let eff_from = (from_elev - mountain_base).max(0.0);
    let eff_to = (to_elev - mountain_base).max(0.0);
    let dh = ((eff_from - eff_to) * OROGRAPHIC_VERTICAL_SCALE).max(0.0);
    if dh > 0.0 { (-dh * k).exp() } else { 1.0 }
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

    // 0. Контракт с SolverPrecalc: upwind_weight_sum[i] — сумма ПОЛОЖИТЕЛЬНЫХ
    // весов на отрезке offs[i]..offs[i+1]. Проверяем один раз в debug,
    // чтобы не ловить тихую ошибку нормировки в релизе.
    #[cfg(debug_assertions)]
    {
        for i in 0..num_cells {
            let start = offs[i];
            let end = offs[i + 1];
            let manual: f32 = w_air[start..end]
                .iter()
                .copied()
                .filter(|&w| w > 0.0)
                .sum();
            debug_assert!(
                (manual - upwind_weight_sum[i]).abs() <= 1e-3 * manual.max(1.0),
                "upwind_weight_sum[{}] = {} не совпадает с суммой положительных весов {}",
                i,
                upwind_weight_sum[i],
                manual,
            );
        }
    }

    // 1. Средняя скорость ветра по планете для устойчивой нормировки Куранта
    let sum_wind: f32 = wind_speed.iter().copied().sum();
    let avg_wind = (sum_wind / num_cells.max(1) as f32).max(1e-4);
    let ref_wind = avg_wind * 1.25;

    // 2. Предрасчет потолка влагоемкости C(T) и стартового пара над океаном
    for i in 0..num_cells {
        let cap = saturation_capacity(snap.temperatures[i]);
        scratch.moisture_capacity[i] = cap;

        if elevs[i] <= 0.0 {
            let open_water = open_water_fraction(snap.albedo[i], config.ocean_albedo);
            scratch.specific_humidity[i] = cap * 0.90 * open_water;
        } else {
            scratch.specific_humidity[i] = 0.0;
        }
    }

    // --- ФИЗИЧЕСКИЕ МАСШТАБЫ В РАДИАНАХ СФЕРЫ ---
    // Полная дистанция переноса за сезон (~1.20 рад ≈ 7600 км при strength = 1.0)
    let total_advection_rad = (1.60 * config.moisture_advection_strength).clamp(0.20, 3.00);
    let num_steps = ((total_advection_rad / (hex_rad * 0.55)).round() as usize).clamp(30, 350);

    // Дистанция, за которую воздух над равнинами теряет 63% влаги (~0.90 рад ≈ 5700 км)
    let drying_scale_rad = (1.20 / config.continental_drying_rate.max(0.05)).clamp(0.15, 8.0);
    let evap_scale_rad = (0.04 / config.ocean_evaporation_rate.max(0.1)).clamp(0.01, 0.8);

    let orographic_k = config.orographic_rain_shadow;
    let mountain_base = config.mountain_base_level;

    // 3. Чистый направленный перенос пара по ветру (Upwind Advection)
    // ВАЖНО: Внутри цикла НЕ разбавляем струю ветра нулями от подветренных соседей (avg_nbr),
    // иначе диффузия назад гасит продвижение фронта вглубь материка!
    for _ in 0..num_steps {
        std::mem::swap(&mut scratch.specific_humidity, &mut scratch.prev_humidity);
        let prev = &scratch.prev_humidity;
        let cap_arr = &scratch.moisture_capacity;
        let out = &mut scratch.specific_humidity;

        out.par_iter_mut().enumerate().for_each(|(i, q_out)| {
            let sp = wind_speed[i];
            let my_elev = elevs[i];
            let my_cap = cap_arr[i];
            let neighbors = &nbrs[i].neighbors;

            let eff_my_elev = (my_elev - mountain_base).max(0.0);

            let norm_sp = (sp / ref_wind).clamp(0.0, 1.35);
            let step_rate = (norm_sp * 0.72).clamp(0.05, 0.85);
            let ds_rad = step_rate * hex_rad;

            let w_sum = upwind_weight_sum[i];
            let incoming_q = if sp > 1e-5 && w_sum > 0.0 {
                let start = offs[i];
                let end = offs[i + 1];
                let mut upwind_q = 0.0;

                for j in start..end {
                    let w = w_air[j];
                    if w > 0.0 {
                        let n_id = neighbors[j - start];
                        let orog = orographic_factor(
                            my_elev,
                            elevs[n_id],
                            mountain_base,
                            orographic_k,
                        );
                        upwind_q += prev[n_id] * orog * w;
                    }
                }
                upwind_q / w_sum
            } else {
                prev[i]
            };

            let mut q = prev[i] * (1.0 - step_rate) + incoming_q * step_rate;

            if my_elev <= 0.0 {
                // ОКЕАН: быстрое насыщение над водой
                let open_water = open_water_fraction(snap.albedo[i], config.ocean_albedo);
                let evap_alpha = 1.0 - (-ds_rad * open_water / evap_scale_rad).exp();
                let target_q = my_cap * 0.95 * open_water;
                q += (target_q - q) * evap_alpha;
            } else {
                // СУША: постепенный расход пара на осадки пропорционально пройденным радианам
                q *= (-ds_rad / drying_scale_rad).exp();

                let evap_suppression =
                    1.0 - (snap.pressure[i].max(0.0) * 1.5).min(1.0);

                // «Нижняя граница подтягивания», а не цель равновесия:
                // испарение только ДОБАВЛЯЕТ пар, поэтому max(0.0) —
                // не даём поправке подсушивать и без того сухой воздух.
                let land_evap_floor = my_cap
                    * 0.3
                    * evap_suppression
                    * (1.0 - (eff_my_elev * 3.0).clamp(0.0, 1.0));
                q += (land_evap_floor - q).max(0.0) * 0.02;
            }

            // Физический предел применим ВЕЗДЕ, не только в горах:
            // иначе пересыщение q > cap теряется безвозвратно при переводе в RH.
            q = q.min(my_cap);

            *q_out = q;
        });
    }

    // 4. Физически масштабируемое сглаживание ПОСЛЕ переноса (ширина ~0.04 рад),
    // чтобы убрать резкие границы струй и дать морскому воздуху мягко зайти на берега.
    //
    // ВАЖНО: это ЧИСТО ГЕОМЕТРИЧЕСКОЕ усреднение, БЕЗ орографического фактора —
    // он уже применён на шаге 3 (адвекция). Повторное применение здесь
    // приводило бы к двойному осушению наветренных склонов.
    let smooth_iters = ((0.04 / hex_rad).round() as usize).clamp(2, 12);
    for _ in 0..smooth_iters {
        std::mem::swap(&mut scratch.specific_humidity, &mut scratch.prev_humidity);
        let prev = &scratch.prev_humidity;
        let out = &mut scratch.specific_humidity;

        out.par_iter_mut().enumerate().for_each(|(i, q_out)| {
            let neighbors = &nbrs[i].neighbors;
            if neighbors.is_empty() {
                *q_out = prev[i];
                return;
            }
            let sum: f32 = neighbors.iter().map(|&n| prev[n]).sum();
            *q_out = prev[i] * 0.6 + (sum / neighbors.len() as f32) * 0.4;
        });
    }

    // 5. Перевод в относительную влажность [0.0 .. 1.0]
    for i in 0..num_cells {
        let cap = scratch.moisture_capacity[i];
        let base_rh = (scratch.specific_humidity[i] / cap).clamp(0.0, 1.0);

        let p = snap.pressure[i].clamp(-1.0, 1.0);
        let pressure_mod = if p < 0.0 {
            1.0 + (-p) * 0.30
        } else {
            1.0 - p * 0.55
        };

        snap.humidity[i] = (base_rh * pressure_mod).clamp(0.0, 1.0);
    }
}