// /src/domains/climate/solver/thermodynamics.rs

use super::smoothstep;
use super::super::climate_config::ClimateConfig;
use crate::{domains::tectonics::tectonics_data::GeologyLayer, world::topology::WorldTopology};

/// Высотное охлаждение с мягким порогом.
/// До base_level — 0, после (base_level + blend_width) — линейно.
pub(super) fn elevation_cooling(elev: f32, base_level: f32, blend_width: f32) -> f32 {
    let x = (elev - base_level).max(0.0);
    let k = blend_width.max(1e-6);
    if x < k {
        x * x / (2.0 * k)
    } else {
        x - k * 0.5
    }
}

pub(super) fn calc_radiative_sea_temp(
    i: usize,
    albedo: f32,
    sun_lat: f32,
    topology: &WorldTopology,
    continentality: &[f32],
    config: &ClimateConfig,
    geology: &GeologyLayer,
) -> f32 {
    let lat = topology.cells[i].lat_lon.x;

    // Инсоляция с учетом смещения зенита Солнца в текущем сезоне
    let base_insolation = (lat - sun_lat).cos().max(0.0);
    let albedo_modifier = if geology.elevations[i] <= 0.0 && albedo < config.land_albedo {
        1.0 // Вся дополнительная поглощенная водой энергия уходит на испарение (будущую влажность), а не в кипяток!
    } else {
        (1.0 - albedo) / (1.0 - config.land_albedo)
    };
    let effective_insolation = base_insolation * albedo_modifier;

    let raw_temp =
        config.pole_temp + (config.equator_temp - config.pole_temp) * effective_insolation;

    let c = continentality[i]; // -1..+1
    let r = config.marginal_sea_moderation_reduction;
    let moderation = if c < 0.0 {
        // open ocean (|c|=1) → reduction=0, coastal (|c|=0) → reduction
        config.base_ocean_moderation * (1.0 - r * (1.0 - c.abs()))
    } else {
        // чтобы не было разрыва на c=0, умножим на (1-r) при c=0
        config.base_ocean_moderation * (1.0 - r) * (1.0 - c)
    };

    // ВАЖНО: Океан обладает термоинерцией и сопротивляется резким сезонным скачкам,
    // но на самой широте остается теплым на экваторе и холодным у полюсов (годовая норма широты),
    // а не стягивает всю планету к +1°C!
    let annual_lat_insolation = lat.cos().max(0.0);
    let ocean_lat_baseline =
        config.pole_temp + (config.equator_temp - config.pole_temp) * annual_lat_insolation * 0.85;
    let global_avg = (config.equator_temp + config.pole_temp) / 2.0;
    let ocean_equilibrium = ocean_lat_baseline * 0.82 + global_avg * 0.18;

    let mut final_sea_temp = raw_temp * (1.0 - moderation) + ocean_equilibrium * moderation;

    // ЭФФЕКТ БАТИМЕТРИИ (Шельф против Глубокого океана):
    // На мелководном шельфе (h от 0.0 до -0.12) тонкий слой воды быстрее вымерзает в полярных широтах.
    // Над глубоководными впадинами и за бровкой шельфа (h < -0.18) огромная масса воды удерживает тепло,
    // останавливая кромку льда вдоль континентального склона!
    let elev = geology.elevations[i];
    if elev <= 0.0 && lat.abs() > 1.0 {
        // 0.0 на мелком шельфе (-0.04), 1.0 в глубоком океане (-0.30)
        let deep_water_factor = smoothstep((elev - (-0.06)) / (-0.22 - (-0.06)));
        // На глубокой воде: +3.9°C защиты; на мелком шельфе: -2.1°C
        let bathymetry_shift = (deep_water_factor - 0.35) * 6.0;
        final_sea_temp += bathymetry_shift;
    }

    final_sea_temp
}

pub(super) fn calc_albedo(temp: f32, elevation: f32, config: &ClimateConfig) -> f32 {
    let base_albedo = if elevation <= 0.0 {
        config.ocean_albedo
    } else {
        config.land_albedo
    };
    let t = (temp - (config.freezing_temp - 4.0)) / 6.0;
    let ice_blend = smoothstep(t);
    config.ice_albedo * (1.0 - ice_blend) + base_albedo * ice_blend
}