use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    render::{
        mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
        render_asset::RenderAssetUsages,
    },
};
use noise::{Fbm, NoiseFn, Perlin};
use rand::RngExt;
use std::f32::consts::PI;

const RADIUS: f32 = 3.0;
const SUBDIVISIONS: u32 = 75; // число сегментов вдоль ребра грани куба
const NUM_PLATES: usize = 15;
const PERC_OF_CONTINENTAL_PLATES: f64 = 0.4;

#[derive(Component)]
struct Globe;

#[derive(PartialEq, Clone, Copy)]
enum PlateType {
    Oceanic,
    Continental,
}

struct Plate {
    center_lat: f32,
    center_lon: f32,
    plate_type: PlateType,
    drift_dir: Vec3, // единичный 3D-вектор, только для dot-произведения
}

// -------------------- Сферические утилиты --------------------

/// (lat, lon) → единичный 3D-вектор.
fn spherical_to_dir(lat: f32, lon: f32) -> Vec3 {
    let (sl, cl) = lat.sin_cos();
    let (so, co) = lon.sin_cos();
    Vec3::new(cl * co, sl, cl * so)
}

/// Единичный 3D-вектор → (lat, lon).
fn dir_to_spherical(d: Vec3) -> (f32, f32) {
    let n = d.normalize_or_zero();
    let lat = n.y.clamp(-1.0, 1.0).asin();
    let lon = n.z.atan2(n.x);
    (lat, lon)
}

/// Угловое расстояние (по большому кругу) между двумя точками сферы, в радианах.
fn angular_distance(lat1: f32, lon1: f32, lat2: f32, lon2: f32) -> f32 {
    let (s1, c1) = lat1.sin_cos();
    let (s2, c2) = lat2.sin_cos();
    let (_, cd) = (lon2 - lon1).sin_cos(); // cos(dlon)
    let cos_d = s1 * s2 + c1 * c2 * cd;
    cos_d.clamp(-1.0, 1.0).acos()
}

/// (face, u, v) → (lat, lon). u, v ∈ [-1, 1].
/// Используются стандартные cubemap-параметризации, дающие одинаковую
/// намотку треугольников на всех гранях.
fn cube_to_spherical(face: u8, u: f32, v: f32) -> (f32, f32) {
    let c = match face {
        0 => Vec3::new(1.0, -v, -u),   // +X
        1 => Vec3::new(-1.0, -v, u),   // -X
        2 => Vec3::new(u, 1.0, v),     // +Y
        3 => Vec3::new(u, -1.0, -v),   // -Y
        4 => Vec3::new(u, -v, 1.0),    // +Z
        _ => Vec3::new(-u, -v, -1.0),  // -Z
    };
    dir_to_spherical(c)
}

/// (lat, lon) → (face, u, v), u, v ∈ [-1, 1]. Полезно для отладки/сэмплинга.
#[allow(dead_code)]
fn spherical_to_cube(lat: f32, lon: f32) -> (u8, f32, f32) {
    let d = spherical_to_dir(lat, lon);
    let ax = d.x.abs();
    let ay = d.y.abs();
    let az = d.z.abs();
    let m = ax.max(ay).max(az);
    if m == ax {
        if d.x > 0.0 { (0, -d.z / m, -d.y / m) } else { (1, d.z / m, -d.y / m) }
    } else if m == ay {
        if d.y > 0.0 { (2, d.x / m, d.z / m) } else { (3, d.x / m, -d.z / m) }
    } else if d.z > 0.0 {
        (4, d.x / m, -d.y / m)
    } else {
        (5, -d.x / m, -d.y / m)
    }
}

// -------------------- Построение меша кубосферы --------------------

fn build_cube_sphere_mesh(radius: f32, subdivisions: u32) -> Mesh {
    let n = subdivisions + 1; // число вершин вдоль ребра одной грани
    let verts_per_face = (n * n) as usize;
    let idx_per_face = ((n - 1) * (n - 1) * 6) as usize;

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(6 * verts_per_face);
    let mut indices: Vec<u32> = Vec::with_capacity(6 * idx_per_face);

    for face in 0..6u8 {
        let base = positions.len() as u32;

        // сетка вершин грани
        for j in 0..n {
            for i in 0..n {
                let u = (i as f32 / (n - 1) as f32) * 2.0 - 1.0;
                let v = (j as f32 / (n - 1) as f32) * 2.0 - 1.0;
                let (lat, lon) = cube_to_spherical(face, u, v);
                let dir = spherical_to_dir(lat, lon);
                positions.push((dir * radius).to_array());
            }
        }

        // индексы (одна и та же намотка для всех граней)
        for j in 0..(n - 1) {
            for i in 0..(n - 1) {
                let a = base + j * n + i;
                let b = a + 1;
                let c = a + n;
                let d = c + 1;
                indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

// -------------------- Приложение --------------------

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup_globe)
        .add_systems(Update, rotate_globe)
        .run();
}

fn setup_globe(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut mesh = build_cube_sphere_mesh(RADIUS, SUBDIVISIONS);

    let mut rng = rand::rng();
    let plates = generate_plates(&mut rng);
    let seed = rng.random_range(0..=u32::MAX);
    let perlin = Fbm::<Perlin>::new(seed);

    let material_handle = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..default()
    });

    apply_tectonic_deformation(&mut mesh, &plates, &perlin);

    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material_handle),
        Globe,
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 12000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 10.0),
    ));
}

fn generate_plates(rng: &mut impl RngExt) -> Vec<Plate> {
    let mut plates = Vec::with_capacity(NUM_PLATES);
    for _ in 0..NUM_PLATES {
        let plate_type = if rng.random_bool(PERC_OF_CONTINENTAL_PLATES) {
            PlateType::Continental
        } else {
            PlateType::Oceanic
        };

        // равномерная выборка точки на сфере: z = sin(lat) однородно на [-1, 1]
        let z: f32 = rng.random_range(-1.0..1.0);
        let lon: f32 = rng.random_range(-PI..PI);
        let lat = z.asin();

        plates.push(Plate {
            center_lat: lat,
            center_lon: lon,
            plate_type,
            drift_dir: Vec3::new(
                rng.random_range(-1.0..1.0),
                rng.random_range(-1.0..1.0),
                rng.random_range(-1.0..1.0),
            )
            .normalize(),
        });
    }
    plates
}

fn apply_tectonic_deformation(mesh: &mut Mesh, plates: &[Plate], noise: &impl NoiseFn<f64, 3>) {
    if let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        let mut new_colors = Vec::with_capacity(positions.len());

        for pos in positions.iter_mut() {
            // 0. Текущая точка → сферические координаты
            let dir = Vec3::from(*pos).normalize();
            let (lat, lon) = dir_to_spherical(dir);

            // 1. Domain warping (в 3D, но результат снова переводим в lat/lon)
            let warp_strength = 0.15;
            let warp_noise = Vec3::new(
                noise.get([dir.x as f64 * 1.5, dir.y as f64 * 1.5, dir.z as f64 * 1.5]) as f32,
                noise.get([dir.y as f64 * 1.5, dir.z as f64 * 1.5, dir.x as f64 * 1.5]) as f32,
                noise.get([dir.z as f64 * 1.5, dir.x as f64 * 1.5, dir.y as f64 * 1.5]) as f32,
            ) * warp_strength;

            let warped_dir = (dir + warp_noise).normalize();
            let (warped_lat, warped_lon) = dir_to_spherical(warped_dir);

            // 2. Поиск ближайших плит через УГЛОВОЕ расстояние
            let mut dist_1 = f32::MAX;
            let mut dist_2 = f32::MAX;
            let mut p1_idx = 0;
            let mut p2_idx = 0;

            for (i, plate) in plates.iter().enumerate() {
                let d = angular_distance(
                    warped_lat,
                    warped_lon,
                    plate.center_lat,
                    plate.center_lon,
                );
                if d < dist_1 {
                    dist_2 = dist_1;
                    p2_idx = p1_idx;
                    dist_1 = d;
                    p1_idx = i;
                } else if d < dist_2 {
                    dist_2 = d;
                    p2_idx = i;
                }
            }

            let p1 = &plates[p1_idx];
            let p2 = &plates[p2_idx];
            let boundary_dist = dist_2 - dist_1;
            let edge_threshold = 0.45;

            // 3. Базовая высота и переходные зоны на границах
            let mut h = if p1.plate_type == PlateType::Continental {
                0.12
            } else {
                -0.35
            };

            if p1.plate_type == PlateType::Continental && p2.plate_type == PlateType::Oceanic {
                let beach_factor = (boundary_dist / 0.2).clamp(0.0, 1.0);
                h = h * beach_factor - (1.0 - beach_factor) * 0.05;
            } else if p1.plate_type == PlateType::Oceanic && p2.plate_type == PlateType::Continental {
                let shelf_factor = (boundary_dist / 0.25).clamp(0.0, 1.0);
                h = h * shelf_factor - (1.0 - shelf_factor) * 0.1;
            } else if p1.plate_type == PlateType::Continental
                && p2.plate_type == PlateType::Continental
            {
                let mountain_factor = (boundary_dist / 0.10).clamp(0.0, 1.0);
                h = h * mountain_factor + (1.0 - mountain_factor) * 0.3;
            } else if p1.plate_type == PlateType::Oceanic && p2.plate_type == PlateType::Oceanic {
                let mountain_factor = (boundary_dist / 0.25).clamp(0.0, 1.0);
                h = h * mountain_factor + (1.0 - mountain_factor) * 0.01;
            }

            // 4. Тектоника (горы, хребты, желоба)
            if boundary_dist < edge_threshold {
                let f = (1.0 - boundary_dist / edge_threshold).clamp(0.0, 1.0);
                let dot = p1.drift_dir.dot(p2.drift_dir);

                let mountain_noise = 0.6
                    + noise.get([
                        warped_dir.x as f64 * 15.0,
                        warped_dir.y as f64 * 15.0,
                        warped_dir.z as f64 * 15.0,
                    ]) as f32
                        * 1.2;

                match (p1.plate_type, p2.plate_type) {
                    (PlateType::Continental, PlateType::Continental) => {
                        if dot < 0.1 {
                            let ridge = (f * PI).sin().powi(2);
                            h += ridge * 0.4 * mountain_noise;
                        }
                    }
                    (PlateType::Continental, PlateType::Oceanic) => {
                        if dot < 0.0 {
                            let m_zone = ((f - 0.2) * 2.0).clamp(0.0, 1.0);
                            let mountain_shape = (m_zone * PI).sin().powi(2);
                            h += mountain_shape * 0.3 * mountain_noise;
                        }
                    }
                    (PlateType::Oceanic, PlateType::Continental) => {
                        if dot < 0.0 {
                            let trench_f = (1.0 - (f - 0.5).abs() * 2.0).clamp(0.0, 1.0);
                            h -= trench_f * 0.25;
                        }
                    }
                    (PlateType::Oceanic, PlateType::Oceanic) => {
                        if dot < -0.2 {
                            let ridge = (f * PI).sin();
                            h += ridge * 0.2 * mountain_noise;
                        }
                    }
                }
            }

            // 5. Финальный шум и клемпы
            let detail_noise = noise.get([
                warped_dir.x as f64 * 4.0,
                warped_dir.y as f64 * 4.0,
                warped_dir.z as f64 * 4.0,
            ]) as f32;
            h += detail_noise * 0.35;

            let final_h = h.max(-0.9);

            // Позицию смещаем по ИСХОДНОМУ направлению (как в оригинале),
            // а warp использовался только для поиска плит.
            *pos = (dir * (RADIUS + final_h)).to_array();

            let color = match final_h {
                x if x <= -0.45 => Color::srgb(0.0, 0.03, 0.12),
                x if x <= -0.18 => Color::srgb(0.01, 0.1, 0.3),
                x if x < 0.0 => Color::srgb(0.05, 0.25, 0.5),
                x if x < 0.035 => Color::srgb(0.85, 0.75, 0.5),
                x if x < 0.18 => Color::srgb(0.2, 0.45, 0.15),
                x if x < 0.4 => Color::srgb(0.4, 0.35, 0.3),
                x if x < 0.6 => Color::srgb(0.3, 0.25, 0.2),
                _ => Color::srgb(0.95, 0.95, 1.0),
            };
            new_colors.push(color.to_linear().to_f32_array());
        }

        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, new_colors);
    }
    mesh.compute_normals();
}

fn rotate_globe(
    mut query: Query<&mut Transform, With<Globe>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    accumulated_mouse: Res<AccumulatedMouseMotion>,
) {
    if mouse_button.pressed(MouseButton::Left)
        && let Ok(mut transform) = query.single_mut()
    {
        let delta = accumulated_mouse.delta;
        let sensitivity = 0.005;
        transform.rotate_y(-delta.x * sensitivity);
        transform.rotate_local_x(-delta.y * sensitivity);
    }
}