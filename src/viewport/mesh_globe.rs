use crate::world::topology::WorldTopology;
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
};

pub fn build_globe_mesh(
    topology: &WorldTopology,
    elevations: &[f32],
    flatten_water: bool,
    cell_colors: &[[f32; 4]],
    radius: f32,
) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for (cell_id, cell) in topology.cells.iter().enumerate() {
        let real_h = elevations[cell_id];
        let render_h = if flatten_water {
            real_h.max(0.0)
        } else {
            real_h
        };
        let r = radius + render_h;
        let top_color = cell_colors[cell_id];
        let top_normal = cell.center.to_array();

        let base_idx = positions.len() as u32;
        positions.push((cell.center * r).to_array());
        normals.push(top_normal);
        colors.push(top_color);

        let n_corners = cell.corners.len();
        for &corner_dir in &cell.corners {
            positions.push((corner_dir * r).to_array());
            normals.push(top_normal);
            colors.push(top_color);
        }

        for k in 0..n_corners as u32 {
            let next_k = (k + 1) % n_corners as u32;
            indices.extend_from_slice(&[base_idx, base_idx + 1 + k, base_idx + 1 + next_k]);
        }

        let wall_color = [
            top_color[0] * 0.75,
            top_color[1] * 0.75,
            top_color[2] * 0.75,
            top_color[3],
        ];

        for k in 0..n_corners {
            let neighbor_id = cell.neighbors[k];
            let real_neighbor_h = elevations[neighbor_id];

            if real_h > real_neighbor_h {
                let render_neighbor_h = if flatten_water {
                    real_neighbor_h.max(0.0)
                } else {
                    real_neighbor_h
                };
                let r_bottom = radius + render_neighbor_h;

                let c1 = cell.corners[k];
                let c2 = cell.corners[(k + 1) % n_corners];

                let v_top_left = c1 * r;
                let v_top_right = c2 * r;
                let v_bot_left = c1 * r_bottom;
                let v_bot_right = c2 * r_bottom;

                let edge_dir = v_top_right - v_top_left;
                let down_dir = v_bot_left - v_top_left;
                let wall_normal = down_dir.cross(edge_dir).normalize_or_zero().to_array();

                let wall_base = positions.len() as u32;
                positions.extend_from_slice(&[
                    v_top_left.to_array(),
                    v_top_right.to_array(),
                    v_bot_left.to_array(),
                    v_bot_right.to_array(),
                ]);
                normals.extend_from_slice(&[wall_normal; 4]);
                colors.extend_from_slice(&[wall_color; 4]);

                indices.extend_from_slice(&[
                    wall_base,
                    wall_base + 2,
                    wall_base + 1,
                    wall_base + 1,
                    wall_base + 2,
                    wall_base + 3,
                ]);
            }
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

pub fn recolor_globe_mesh(
    topology: &WorldTopology,
    elevations: &[f32],
    cell_colors: &[[f32; 4]],
) -> Vec<[f32; 4]> {
    // Резервируем память сразу, чтобы избежать фризов от реаллокации
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(3_200_000);

    for (cell_id, cell) in topology.cells.iter().enumerate() {
        let h = elevations[cell_id];
        let top_color = cell_colors[cell_id];

        // Цвет центра
        colors.push(top_color);

        // Цвета углов
        let n_corners = cell.corners.len();
        for _ in 0..n_corners {
            colors.push(top_color);
        }

        // Цвета отвесных стен (затемненные)
        let wall_color = [
            top_color[0] * 0.75,
            top_color[1] * 0.75,
            top_color[2] * 0.75,
            top_color[3],
        ];

        for k in 0..n_corners {
            let neighbor_id = cell.neighbors[k];
            let neighbor_h = elevations[neighbor_id];

            if h > neighbor_h {
                // Если есть стена, она состоит из 4 вершин (quad)
                colors.extend_from_slice(&[wall_color; 4]);
            }
        }
    }

    colors
}

pub fn reposition_globe_mesh(
    topology: &WorldTopology,
    real_elevations: &[f32], // Реальные высоты (нужны, чтобы условие стен совпало!)
    flatten_water: bool,     // Выравнивать ли воду
    radius: f32,
) -> Vec<[f32; 3]> {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(3_200_000);

    for (cell_id, cell) in topology.cells.iter().enumerate() {
        let real_h = real_elevations[cell_id];
        let render_h = if flatten_water {
            real_h.max(0.0)
        } else {
            real_h
        };
        let r = radius + render_h;

        // Центр и углы
        positions.push((cell.center * r).to_array());
        let n_corners = cell.corners.len();
        for &corner_dir in &cell.corners {
            positions.push((corner_dir * r).to_array());
        }

        // Стены: ВАЖНО проверять по real_h, чтобы число вершин не изменилось!
        for k in 0..n_corners {
            let neighbor_id = cell.neighbors[k];
            let real_neighbor_h = real_elevations[neighbor_id];

            if real_h > real_neighbor_h {
                let render_neighbor_h = if flatten_water {
                    real_neighbor_h.max(0.0)
                } else {
                    real_neighbor_h
                };
                let r_bottom = radius + render_neighbor_h;

                let c1 = cell.corners[k];
                let c2 = cell.corners[(k + 1) % n_corners];

                positions.extend_from_slice(&[
                    (c1 * r).to_array(),
                    (c2 * r).to_array(),
                    (c1 * r_bottom).to_array(),
                    (c2 * r_bottom).to_array(),
                ]);
            }
        }
    }

    positions
}
