// /src/domains/climate/solver/thermodynamics.rs

use super::{smoothstep, SolverPrecalc};
use super::super::climate_config::ClimateConfig;
use crate::domains::tectonics::tectonics_data::GeologyLayer;

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

#[inline]
pub(super) fn calc_radiative_sea_temp(
    i: usize,
    albedo: f32,
    base_insolation: f32,
    precalc: &SolverPrecalc,
    config: &ClimateConfig,
    geology: &GeologyLayer,
) -> f32 {
    let albedo_modifier = if geology.elevations[i] <= 0.0 && albedo < config.land_albedo {
        1.0
    } else {
        (1.0 - albedo) / (1.0 - config.land_albedo)
    };
    let effective_insolation = base_insolation * albedo_modifier;

    let raw_temp =
        config.pole_temp + (config.equator_temp - config.pole_temp) * effective_insolation;

    let moderation = precalc.moderation[i];
    let ocean_equilibrium = precalc.ocean_equilibrium[i];

    raw_temp * (1.0 - moderation) + ocean_equilibrium * moderation + precalc.bathymetry_shift[i]
}

#[inline]
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