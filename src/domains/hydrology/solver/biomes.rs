// src/domains/hydrology/solver/biomes.rs

use crate::{
    domains::{
        climate::climate_data::ClimateLayer,
        hydrology::{
            hydrology_config::HydrologyConfig,
            hydrology_data::{Biome, HydrologyLayer, SoilType},
        },
        tectonics::tectonics_data::GeologyLayer,
    },
    world::topology::WorldTopology,
};

pub(super) fn compute_biomes(
    topology: &WorldTopology,
    geology: &GeologyLayer,
    climate: &ClimateLayer,
    hydrology: &mut HydrologyLayer,
    config: &HydrologyConfig,
) {
    let num_cells = topology.cells.len();

    for i in 0..num_cells {
        let elev = geology.elevations[i];

        // 1. Сезонные температуры и осадки
        let t_spring = climate.spring.temperatures[i];
        let t_summer = climate.summer.temperatures[i];
        let t_winter = climate.winter.temperatures[i];

        let mean_temp = (2.0 * t_spring + t_summer + t_winter) * 0.25;
        let warm_temp = t_summer.max(t_winter).max(t_spring);
        let cold_temp = t_summer.min(t_winter).min(t_spring);

        // 2. Океанические биомы
        if elev <= 0.0 {
            hydrology.biomes[i] = if mean_temp < -4.5 || warm_temp < -1.0 {
                Biome::SeaIce
            } else if elev > -0.09 {
                Biome::ShallowOcean
            } else {
                Biome::DeepOcean
            };
            continue;
        }

        let p_spring = climate.spring.precipitation[i];
        let p_summer = climate.summer.precipitation[i];
        let p_winter = climate.winter.precipitation[i];
        let annual_precip = (2.0 * p_spring + p_summer + p_winter) * 0.25;

        // Определяем осадки самого теплого и самого холодного сезона (для муссонов и Средиземноморья)
        let (warm_precip, cold_precip) = if t_summer >= t_winter {
            (p_summer, p_winter)
        } else {
            (p_winter, p_summer)
        };

        // 3. Рельеф (уклон)
        let neighbors = &topology.cells[i].neighbors;
        let mut max_slope = 0.0_f32;
        for &n_id in neighbors {
            let diff = (elev - geology.elevations[n_id]).abs();
            if diff > max_slope {
                max_slope = diff;
            }
        }

        let soil = hydrology.soils[i];
        let [sand, _silt, clay] = hydrology.soil_texture[i];
        let whc = hydrology.water_capacity[i];
        let flow = hydrology.flow_accumulation[i];

        // 4. Криосфера и высокогорные скалы
        // Вечные льды (ледниковые щиты и снежные вершины)
        if warm_temp < 0.0 || (mean_temp < -9.0 && warm_temp < 3.5) {
            hydrology.biomes[i] = Biome::IceSheet;
            continue;
        }

        // Высокогорные скалы и осыпи (крутые склоны на Leptosol или выше линии леса)
        if soil == SoilType::Leptosol && (elev > 0.22 || max_slope > config.mountain_slope_threshold * 1.15) {
            hydrology.biomes[i] = Biome::AlpineBare;
            continue;
        }

        // Тундра (короткое холодное лето, деревья не могут вырасти, мерзлотный Gelisol)
        if warm_temp < 8.5 || soil == SoilType::Gelisol {
            hydrology.biomes[i] = if max_slope > config.mountain_slope_threshold {
                Biome::AlpineBare
            } else {
                Biome::Tundra
            };
            continue;
        }

        // 5. Почвенная поправка к доступной влаге (эффект песка/глины на диаграмму Уиттекера)
        // Базовая WHC суглинка ~ 0.32. Песок (низкая WHC) сушит биом, суглинок/андисоль — увлажняет.
        let soil_moisture_mod = 1.0 + (whc - 0.30) * config.soil_biome_influence
            - (sand - 0.45).max(0.0) * 0.45 * config.soil_biome_influence;

        let effective_precip = (annual_precip * soil_moisture_mod.clamp(0.55, 1.45)).max(0.0);

        // Потенциальное испарение (PET) и индекс доступного увлажнения (MI = P_eff / PET)
        let pet = ((mean_temp + 10.0).max(2.0) * 0.021 * config.aridity_evap_scale).max(0.04);
        let mut moisture_idx = effective_precip / pet;

        // 6. Азональные гидрологические биомы: Болота (Wetland) и Речные Оазисы (Riparian)
        let is_major_river = flow >= config.river_threshold * config.riparian_river_multiplier;

        // Болота: плоские глинистые или глеевые низины с застоем воды или разливом крупных рек
        if max_slope < config.wetland_max_slope
            && elev < 0.12
            && (soil == SoilType::Gleysol || clay > 0.38)
            && (moisture_idx > 0.78 || (is_major_river && moisture_idx > 0.42))
        {
            hydrology.biomes[i] = Biome::Wetland;
            continue;
        }

        // Пойменный оазис / галерейный лес вдоль крупной реки в пустыне или полупустыне
        if is_major_river && moisture_idx < 0.35 && mean_temp > 5.0 {
            hydrology.biomes[i] = Biome::RiparianOasis;
            continue;
        }

        // В степях и саваннах крупная река подпитывает долину грунтовыми водами
        if is_major_river {
            moisture_idx += 0.25;
        }

        // 7. Классификация Уиттекера с учетом сезонности
        hydrology.biomes[i] = classify_whittaker_biome(
            mean_temp,
            warm_temp,
            cold_temp,
            moisture_idx,
            warm_precip,
            cold_precip,
            sand,
            soil,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn classify_whittaker_biome(
    mean_temp: f32,
    warm_temp: f32,
    cold_temp: f32,
    moisture_idx: f32,
    warm_precip: f32,
    cold_precip: f32,
    sand: f32,
    soil: SoilType,
) -> Biome {
    // --- А. ЭКСТРЕМАЛЬНО СУХИЕ ЗОНЫ (Пустыни и полупустыни) ---
    if moisture_idx < 0.18 {
        return if mean_temp < 11.0 || cold_temp < -5.0 {
            Biome::ColdDesert
        } else {
            Biome::HotDesert
        };
    }

    if moisture_idx < 0.33 {
        return if mean_temp < 9.0 || cold_temp < -6.0 {
            // Переходная холодная полупустыня / сухая степь
            if soil == SoilType::Chernozem {
                Biome::TemperateGrassland
            } else {
                Biome::ColdDesert
            }
        } else {
            Biome::XericShrubland
        };
    }

    // --- Б. БОРЕАЛЬНЫЙ ПОЯС (Холодная зима или прохладное лето -> Тайга / Степь) ---
    if mean_temp < 5.5 || cold_temp < -12.0 {
        return if moisture_idx < 0.55 && soil != SoilType::Podzol {
            Biome::TemperateGrassland
        } else {
            Biome::BorealTaiga
        };
    }

    // --- В. УМЕРЕННЫЙ И СУБТРОПИЧЕСКИЙ ПОЯС (5.5°C .. 19.5°C) ---
    if mean_temp < 19.5 {
        // Проверка на средиземноморский режим: теплая зима (> 2°C), жаркое сухое лето, дождливая зима
        let is_mediterranean = cold_temp > 1.5
            && warm_temp > 18.0
            && cold_precip > warm_precip * 1.65
            && moisture_idx < 0.85;

        if is_mediterranean {
            return Biome::MediterraneanShrubland;
        }

        if moisture_idx < 0.58 {
            return Biome::TemperateGrassland;
        }

        // На очень бедных песчаных подзолах даже в умеренном климате растут сосновые боры (Тайга)
        if soil == SoilType::Podzol && sand > 0.55 && mean_temp < 11.0 {
            return Biome::BorealTaiga;
        }

        return if moisture_idx > 1.35 && cold_temp > -2.0 {
            Biome::TemperateRainforest
        } else {
            Biome::TemperateDeciduousForest
        };
    }

    // --- Г. ТРОПИЧЕСКИЙ И ЭКВАТОРИАЛЬНЫЙ ПОЯС (> 19.5°C) ---
    // Коэффициент сухого сезона (отношение минимальных сезонных осадков к максимальным)
    let p_min = warm_precip.min(cold_precip);
    let p_max = warm_precip.max(cold_precip).max(1e-4);
    let seasonality_ratio = p_min / p_max;

    // Если почва сильно песчаная или влаги умеренно -> Саванна
    if moisture_idx < 0.72 || (moisture_idx < 0.95 && seasonality_ratio < 0.35) {
        Biome::Savanna
    } else if moisture_idx < 1.18 || seasonality_ratio < 0.45 {
        Biome::TropicalSeasonalForest
    } else {
        Biome::TropicalRainforest
    }
}