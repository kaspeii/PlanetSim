// src/domains/hydrology/solver/rivers.rs

use std::{cmp::Ordering, collections::BinaryHeap, f32::consts::PI};

use crate::{
    domains::{
        climate::climate_data::ClimateLayer,
        hydrology::{
            hydrology_config::HydrologyConfig,
            hydrology_data::{HydrologyLayer, SoilType},
        },
        tectonics::tectonics_data::GeologyLayer,
    },
    world::topology::WorldTopology,
};

#[derive(Copy, Clone, PartialEq)]
struct FloodNode {
    elev: f32,
    cell_id: usize,
}

impl Eq for FloodNode {}

// Min-Heap: первыми извлекаются клетки с наименьшей высотой
impl Ord for FloodNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .elev
            .total_cmp(&self.elev)
            .then_with(|| self.cell_id.cmp(&other.cell_id))
    }
}

impl PartialOrd for FloodNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(super) fn compute_rivers(
    topology: &WorldTopology,
    geology: &GeologyLayer,
    climate: &ClimateLayer,
    hydrology: &mut HydrologyLayer,
    config: &HydrologyConfig,
) {
    let num_cells = topology.cells.len();
    if num_cells == 0 {
        return;
    }

    // Физическая площадь одной ячейки на единичной сфере (обеспечивает масштабонезависимость)
    let cell_area = 4.0 * PI / (num_cells as f32);

    // =========================================================================
    // 1. РАСЧЕТ ПОЛЯ СТОКА (Runoff = Осадки - Эвапотранспирация Будыко + Таяние снега)
    // =========================================================================
    for i in 0..num_cells {
        if geology.elevations[i] <= 0.0 {
            hydrology.runoff[i] = 0.0;
            hydrology.flow_dir[i] = None;
            hydrology.flow_accumulation[i] = 0.0;
            continue;
        }

        let t_spring = climate.spring.temperatures[i];
        let t_summer = climate.summer.temperatures[i];
        let t_winter = climate.winter.temperatures[i];
        let mean_temp = (2.0 * t_spring + t_summer + t_winter) * 0.25;

        let p_spring = climate.spring.precipitation[i];
        let p_summer = climate.summer.precipitation[i];
        let p_winter = climate.winter.precipitation[i];
        let annual_precip = (2.0 * p_spring + p_summer + p_winter) * 0.25;

        // Зимний снег, который тает весной/летом, дает залповый сток (не успевая испариться)
        let cold_temp = t_winter.min(t_spring);
        let warm_temp = t_summer.max(t_spring);
        let cold_precip = if t_winter < t_summer { p_winter } else { p_summer };
        let snow_share = ClimateLayer::snow_fraction(cold_temp);
        let melt_factor = ((warm_temp - 0.0) / 8.0).clamp(0.0, 1.0);
        let snowmelt_bonus = cold_precip * snow_share * melt_factor * 0.30;

        // Испаряемость (PET) и влагоудержание почвы (WHC):
        // Глинистые глубокие почвы удерживают влагу у корней и испаряют больше;
        // скалы (Leptosol) и пески быстро сбрасывают воду в сток
        let whc = hydrology.water_capacity[i];
        let pet = ((mean_temp + 10.0).max(0.5) * 0.016 * config.aridity_evap_scale).max(0.02);
        let aridity_index = (pet / annual_precip.max(1e-4)) * (0.55 + 0.65 * whc);

        // Кривая Будыко для фактической эвапотранспирации (AET)
        let aet_ratio = aridity_index / (1.0 + aridity_index * aridity_index).sqrt();
        let net_rain_runoff = annual_precip * (1.0 - aet_ratio).clamp(0.05, 0.95);

        // Потери в глубокие карстовые горизонты (известняки)
        let karst_loss = geology.lithology[i].karst_permeability() * 0.25;

        let raw_runoff = (net_rain_runoff + snowmelt_bonus) * (1.0 - karst_loss);
        hydrology.runoff[i] = (raw_runoff * config.runoff_multiplier).max(0.0);
    }

    // =========================================================================
    // 2. ТРАССИРОВКА НАПРАВЛЕНИЙ СТОКА (Priority-Flood + Эродируемость пород)
    // =========================================================================
    let mut visited = vec![false; num_cells];
    let mut filled_elev = vec![0.0_f32; num_cells];
    let mut heap = BinaryHeap::with_capacity(num_cells / 4);
    let mut downstream_order = Vec::with_capacity(num_cells);

    // Инициализируем океан как базовый уровень стока
    for i in 0..num_cells {
        let h = geology.elevations[i];
        if h <= 0.0 {
            visited[i] = true;
            filled_elev[i] = h;
            heap.push(FloodNode { elev: h, cell_id: i });
        }
    }

    // Обходим планету от береговой линии вверх к водоразделам:
    // мягкие породы (ShaleClay, Sandstone) имеют меньшее эффективное сопротивление,
    // поэтому реки естественно стремятся течь по осадочным долинам, огибая гранитные массивы.
    while let Some(FloodNode { cell_id: u, .. }) = heap.pop() {
        if geology.elevations[u] > 0.0 {
            downstream_order.push(u);

            // Если клетка не в замкнутой яме, уточняем направление по самому крутому уклону
            // среди уже открытых более низких соседей (убирает сеточную угловатость BFS)
            let my_filled = filled_elev[u];
            let neighbors = &topology.cells[u].neighbors;
            let mut best_k = hydrology.flow_dir[u];
            let mut max_drop = -1.0_f32;

            for (k, &n_id) in neighbors.iter().enumerate() {
                if visited[n_id] && filled_elev[n_id] < my_filled {
                    let rock_bonus =
                        geology.lithology[n_id].erodibility() * config.lithology_erosion_bias;
                    let drop = (my_filled - filled_elev[n_id]) + rock_bonus;
                    if drop > max_drop {
                        max_drop = drop;
                        best_k = Some(k as u8);
                    }
                }
            }
            hydrology.flow_dir[u] = best_k;
        }

        let u_filled = filled_elev[u];
        for &v in &topology.cells[u].neighbors {
            if !visited[v] {
                visited[v] = true;

                // Находим индекс грани `k_back`, ведущей из `v` обратно в `u`
                let k_back = topology.cells[v]
                    .neighbors
                    .iter()
                    .position(|&id| id == u)
                    .unwrap_or(0) as u8;
                hydrology.flow_dir[v] = Some(k_back);

                // Эффективная высота с учетом податливости породы размыву
                let erod_offset =
                    geology.lithology[v].erodibility() * config.lithology_erosion_bias;
                let raw_v_elev = geology.elevations[v] - erod_offset;
                let v_filled = raw_v_elev.max(u_filled + 1e-5);
                filled_elev[v] = v_filled;

                heap.push(FloodNode {
                    elev: v_filled,
                    cell_id: v,
                });
            }
        }
    }

    // =========================================================================
    // 3. АККУМУЛЯЦИЯ ПОТОКА (от вершин к океану) и ОБРАТНАЯ СВЯЗЬ С ПОЙМАМИ
    // =========================================================================
    for i in 0..num_cells {
        hydrology.flow_accumulation[i] = if geology.elevations[i] > 0.0 {
            hydrology.runoff[i] * cell_area
        } else {
            0.0
        };
    }

    // Идем в обратном порядке (от самых высоких горных истоков вниз к устьям рек)
    for &u in downstream_order.iter().rev() {
        let flow = hydrology.flow_accumulation[u];
        if let Some(k) = hydrology.flow_dir[u] {
            let target = topology.cells[u].neighbors[k as usize];
            if geology.elevations[target] > 0.0 {
                hydrology.flow_accumulation[target] += flow;
            }
        }

        // Крупные реки на пологих равнинах формируют аллювиально-пойменные почвы (Gleysol)
        // и повышают влагоемкость долины для будущего этапа биомов (пойменные леса и оазисы)
        if flow >= config.river_threshold * 2.5 && geology.elevations[u] < 0.14 {
            hydrology.water_capacity[u] = (hydrology.water_capacity[u] + 0.30).clamp(0.0, 1.0);
            if hydrology.soils[u] == SoilType::Aridisol || hydrology.soils[u] == SoilType::Cambisol {
                hydrology.soils[u] = SoilType::Gleysol;
            }
        }
    }
}