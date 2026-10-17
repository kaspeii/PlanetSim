use crate::grid::WorldGrid;
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
};

pub fn build_hex_mesh(grid: &WorldGrid, radius: f32) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for cell in &grid.cells {
        let h = cell.elevation;
        let r = radius + h;
        let top_color = elevation_to_color(h).to_linear().to_f32_array();
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

        let wall_color = [top_color[0] * 0.75, top_color[1] * 0.75, top_color[2] * 0.75, top_color[3]];

        for k in 0..n_corners {
            let neighbor_id = cell.neighbors[k];
            let neighbor_h = grid.cells[neighbor_id].elevation;

            if h > neighbor_h {
                let r_bottom = radius + neighbor_h;
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
                positions.extend_from_slice(&[v_top_left.to_array(), v_top_right.to_array(), v_bot_left.to_array(), v_bot_right.to_array()]);
                normals.extend_from_slice(&[wall_normal; 4]);
                colors.extend_from_slice(&[wall_color; 4]);

                indices.extend_from_slice(&[wall_base, wall_base + 2, wall_base + 1, wall_base + 1, wall_base + 2, wall_base + 3]);
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn elevation_to_color(h: f32) -> Color {
    match h {
        x if x <= -0.45 => Color::srgb(0.0, 0.03, 0.12),
        x if x <= -0.18 => Color::srgb(0.01, 0.1, 0.3),
        x if x < 0.0 => Color::srgb(0.05, 0.25, 0.5),
        x if x < 0.035 => Color::srgb(0.85, 0.75, 0.5),
        x if x < 0.18 => Color::srgb(0.2, 0.45, 0.15),
        x if x < 0.4 => Color::srgb(0.4, 0.35, 0.3),
        x if x < 0.6 => Color::srgb(0.3, 0.25, 0.2),
        _ => Color::srgb(0.95, 0.95, 1.0),
    }
}