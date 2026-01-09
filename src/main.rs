use bevy::{input::mouse::AccumulatedMouseMotion, prelude::*, render::mesh::VertexAttributeValues};
use noise::{Fbm, NoiseFn, Perlin};
use rand::Rng;

const RADIUS: f32 = 3.0;
const SUBDIVISIONS: u32 = 75; // 80 - это максимум для стандартного билдера Bevy
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
    center: Vec3,
    plate_type: PlateType,
    drift_dir: Vec3,
}

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
    // Используем 75, чтобы не паниковать по лимиту вершин
    let mut mesh = Sphere::new(RADIUS).mesh().ico(SUBDIVISIONS).unwrap();

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
        MeshMaterial3d(material_handle.clone()),
        Globe,
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 12000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((Camera3d::default(), Transform::from_xyz(0.0, 0.0, 10.0)));
}

fn generate_plates(rng: &mut impl Rng) -> Vec<Plate> {
    let mut plates = Vec::with_capacity(NUM_PLATES as usize);
    for _ in 0..NUM_PLATES {
        let plate_type = if rng.random_bool(PERC_OF_CONTINENTAL_PLATES) {
            PlateType::Continental
        } else {
            PlateType::Oceanic
        };
        plates.push(Plate {
            center: Vec3::new(
                rng.random_range(-1.0..1.0),
                rng.random_range(-1.0..1.0),
                rng.random_range(-1.0..1.0),
            )
            .normalize(),
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
            let v = Vec3::from(*pos).normalize();

            // --- 1. ИСКАЖЕНИЕ ГРАНИЦ (Domain Warping) ---
            // Мы добавляем шум к позиции ПЕРЕД поиском ближайшей плиты.
            // Это сделает границы "рваными" и скругленными.
            let warp_strength = 0.15;
            let warp_noise = Vec3::new(
                noise.get([v.x as f64 * 1.5, v.y as f64 * 1.5, v.z as f64 * 1.5]) as f32,
                noise.get([v.y as f64 * 1.5, v.z as f64 * 1.5, v.x as f64 * 1.5]) as f32,
                noise.get([v.z as f64 * 1.5, v.x as f64 * 1.5, v.y as f64 * 1.5]) as f32,
            ) * warp_strength;

            let warped_v = (v + warp_noise).normalize();

            // --- 2. ПОИСК БЛИЖАЙШИХ ПЛИТ (по искаженным координатам) ---
            let mut dist_1 = f32::MAX;
            let mut dist_2 = f32::MAX;
            let mut p1_idx = 0;
            let mut p2_idx = 0;

            for (i, plate) in plates.iter().enumerate() {
                let d = warped_v.distance(plate.center);
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

            // --- 3. БАЗОВАЯ ВЫСОТА И ПЛЯЖИ ---
            let mut h = if p1.plate_type == PlateType::Continental {
                0.12
            } else {
                -0.35
            };

            // Если мы на континенте, а рядом океан — создаем склон к пляжу
            if p1.plate_type == PlateType::Continental && p2.plate_type == PlateType::Oceanic {
                let beach_factor = (boundary_dist / 0.2).clamp(0.0, 1.0);
                // Плавно опускаем высоту от 0.12 до -0.05 при приближении к границе
                h = h * beach_factor - (1.0 - beach_factor) * 0.05;
            }
            // Если мы в океане, а рядом континент — создаем мелководье (шельф)
            else if p1.plate_type == PlateType::Oceanic && p2.plate_type == PlateType::Continental
            {
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

            // --- 4. ТЕКТОНИКА (ГОРЫ И ВПАДИНЫ) ---
            if boundary_dist < edge_threshold {
                let f = (1.0 - boundary_dist / edge_threshold).clamp(0.0, 1.0);
                let dot = p1.drift_dir.dot(p2.drift_dir);

                let mountain_noise = 0.6
                    + noise.get([v.x as f64 * 15.0, v.y as f64 * 15.0, v.z as f64 * 15.0]) as f32
                        * 1.2;

                match (p1.plate_type, p2.plate_type) {
                    (PlateType::Continental, PlateType::Continental) => {
                        if dot < 0.1 {
                            // Высокий хребет на стыке двух континентов
                            let ridge = (f * std::f32::consts::PI).sin().powi(2);
                            h += ridge * 0.4 * mountain_noise;
                        }
                    }
                    (PlateType::Continental, PlateType::Oceanic) => {
                        if dot < 0.0 {
                            // Горы на удалении от берега (Субдукция)
                            // Сдвигаем пик гор вглубь континента (f от 0.2 до 0.8)
                            let m_zone = ((f - 0.2) * 2.0).clamp(0.0, 1.0);
                            let mountain_shape = (m_zone * std::f32::consts::PI).sin().powi(2);
                            h += mountain_shape * 0.3 * mountain_noise;
                        }
                    }
                    (PlateType::Oceanic, PlateType::Continental) => {
                        if dot < 0.0 {
                            // Океанический желоб прямо у границы
                            let trench_f = (1.0 - (f - 0.5).abs() * 2.0).clamp(0.0, 1.0);
                            h -= trench_f * 0.25;
                        }
                    }
                    (PlateType::Oceanic, PlateType::Oceanic) => {
                        if dot < -0.2 {
                            // Срединный хребет
                            let ridge = (f * std::f32::consts::PI).sin();
                            h += ridge * 0.2 * mountain_noise;
                        }
                    }
                }
            }

            // --- 5. ФИНАЛЬНЫЙ ШУМ И КЛЕМПЫ ---
            let detail_noise =
                noise.get([v.x as f64 * 4.0, v.y as f64 * 4.0, v.z as f64 * 4.0]) as f32;
            h += detail_noise * 0.35;

            let final_h = h.max(-0.9);
            let visual_h = final_h;
            // let visual_h = if final_h < 0.0 { 0.0 } else { final_h };
            *pos = (v * (RADIUS + visual_h)).to_array();

            let color = match final_h {
                x if x <= -0.45 => Color::srgb(0.0, 0.03, 0.12), // Глубокие желоба
                x if x <= -0.18 => Color::srgb(0.01, 0.1, 0.3),  // Океан
                x if x < 0.0 => Color::srgb(0.05, 0.25, 0.5),    // Мелководье
                x if x < 0.035 => Color::srgb(0.85, 0.75, 0.5),  // Пляж (Песок)
                x if x < 0.18 => Color::srgb(0.2, 0.45, 0.15),   // Равнина (Зелень)
                x if x < 0.4 => Color::srgb(0.4, 0.35, 0.3),     // Горы
                x if x < 0.6 => Color::srgb(0.3, 0.25, 0.2),     // Высокие скалы
                _ => Color::srgb(0.95, 0.95, 1.0),               // Снег
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
