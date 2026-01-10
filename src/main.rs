use bevy::{input::mouse::AccumulatedMouseMotion, prelude::*, mesh::VertexAttributeValues};
use noise::{Fbm, NoiseFn, Perlin};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

const RADIUS: f32 = 3.0;
const SUBDIVISIONS: u32 = 75; // 80 - это максимум для стандартного билдера Bevy
const NUM_PLATES: usize = 15;
const PERC_OF_CONTINENTAL_PLATES: f64 = 0.4;

#[derive(Resource)]
struct WorldSettings {
    seed: Option<u32>,
}

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
        .insert_resource(WorldSettings { seed: Some(144) }) 
        .add_systems(Startup, setup_globe)
        .add_systems(Update, rotate_globe)
        .run();
}

fn setup_globe(
    mut commands: Commands,
    settings: Res<WorldSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Используем 75, чтобы не паниковать по лимиту вершин
    let mut mesh = Sphere::new(RADIUS).mesh().ico(SUBDIVISIONS).unwrap();

    let seed = match settings.seed {
            Some(s) => s,
            None => rand::rng().random_range(0..=u32::MAX),
    };

    let mut rng = ChaCha8Rng::seed_from_u64(seed as u64);

    let plates = generate_plates(&mut rng);

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
    let mut plates = Vec::with_capacity(NUM_PLATES);
    let min_distance = 0.4;
    

    while plates.len() < NUM_PLATES as usize {
        let new_center = Vec3::new(
            rng.random_range(-1.0..1.0),
            rng.random_range(-1.0..1.0),
            rng.random_range(-1.0..1.0),
        ).normalize();

        // Проверяем, нет ли рядом соседа
        let is_too_close = plates.iter().any(|p: &Plate| p.center.distance(new_center) < min_distance);

        if !is_too_close {
            plates.push(Plate {
                center: new_center,
                plate_type: if rng.random_bool(PERC_OF_CONTINENTAL_PLATES) { PlateType::Continental } else { PlateType::Oceanic },
                drift_dir: Vec3::new(
                    rng.random_range(-1.0..1.0),
                    rng.random_range(-1.0..1.0),
                    rng.random_range(-1.0..1.0),
                )
                .normalize(),
            });
        }
    }
    plates
}

fn apply_tectonic_deformation(mesh: &mut Mesh, plates: &[Plate], noise: &impl NoiseFn<f64, 3>) {
    let k = 25.0; 
    let world_radius = RADIUS;

    // Извлекаем позиции для изменения и готовим вектор для цветов
    if let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        let mut new_colors = Vec::with_capacity(positions.len());

        for pos in positions.iter_mut() {
            let v = Vec3::from(*pos);
            let v_norm = v.normalize();

            // --- 1. ВЫЧИСЛЯЕМ ВЕСА ПЛИТ (Softmax) ---
            let mut weights = Vec::with_capacity(plates.len());
            let mut sum_weight = 0.0;
            for plate in plates {
                let dist = v_norm.dot(plate.center); 
                let w = (k * dist).exp();
                weights.push(w);
                sum_weight += w;
            }
            for w in weights.iter_mut() { *w /= sum_weight; }

            // --- 2. БАЗОВАЯ ВЫСОТА ---
            let mut h_base = 0.0;
            for (i, w) in weights.iter().enumerate() {
                let plate_h = if plates[i].plate_type == PlateType::Continental { 0.08 } else { -0.35 };
                h_base += w * plate_h;
            }

// --- 3. ТЕКТОНИЧЕСКИЙ СТРЕСС (Матрица взаимодействий) ---
            let mut stress = 0.0;
            for i in 0..plates.len() {
                if weights[i] < 0.01 { continue; }
                for j in (i + 1)..plates.len() {
                    if weights[j] < 0.01 { continue; }
                    
                    // Сила взаимодействия между парой плит в данной точке
                    let w_pair = weights[i] * weights[j] * 4.0; 
                    let dot_drift = plates[i].drift_dir.dot(plates[j].drift_dir);
                    
                    match (plates[i].plate_type, plates[j].plate_type) {
                        // 1. КОНТИНЕНТ - КОНТИНЕНТ (Орогенез / Рифт)
                        (PlateType::Continental, PlateType::Continental) => {
                            if dot_drift < -0.1 {
                                stress += w_pair * 0.55; // Мощные горы (Гималаи)
                            } else if dot_drift > 0.1 {
                                stress -= w_pair * 0.35; // Рифтовая долина (Африканский разлом)
                            }
                        }

                        // 2. КОНТИНЕНТ - ОКЕАН (Субдукция)
                        (PlateType::Continental, PlateType::Oceanic) | (PlateType::Oceanic, PlateType::Continental) => {
                            if dot_drift < -0.1 {
                                // Определяем, кто есть кто
                                let (w_cont, w_ocean) = if plates[i].plate_type == PlateType::Continental {
                                    (weights[i], weights[j])
                                } else {
                                    (weights[j], weights[i])
                                };

                                // --- МАГИЯ ПЛЯЖА ---
                                // Разница весов: на границе diff = 0. 
                                // На континенте diff > 0, в океане diff < 0.
                                let diff = w_cont - w_ocean;

                                // "Профиль субдукции": 
                                // Умножаем w_pair (который стягивает всё к границе) 
                                // на diff (который в центре равен 0 и растет в стороны).
                                // Это создает "S-образную" кривую: пик вверх -> ноль -> пик вниз.
                                let subduction_profile = diff.powi(5) * w_pair * 2.0;

                                // Увеличиваем множители, так как перемножение весов уменьшает итоговое число
                                stress += subduction_profile * 0.8; 
                            } else {
                                // Пассивный разрыв
                                stress -= w_pair * 0.1;
                            }
                        }

                        // 3. ОКЕАН - ОКЕАН (Дуги / Срединные хребты)
                        (PlateType::Oceanic, PlateType::Oceanic) => {
                            if dot_drift < -0.1 {
                                // Столкновение океанов — островные дуги (Япония, Марианские о-ва)
                                stress += w_pair * 0.6; 
                            } else if dot_drift > 0.1 {
                                // Срединно-океанический хребет — магма поднимает дно
                                stress += w_pair * 0.2; 
                            }
                        }
                    }
                }
            }

            // --- 4. НАСТРОЙКА ШУМОВ ---
            
            // А. Общий шум поверхности (низкая частота, мягкий)
            let surf_freq = 1.5;
            let surface_noise = noise.get([
                v.x as f64 * surf_freq, 
                v.y as f64 * surf_freq, 
                v.z as f64 * surf_freq
            ]) as f32;

            // Б. Горный шум (высокая частота, "гребневый")
            let mount_freq = 0.5;
            let raw_mount_noise = noise.get([
                v.x as f64 * mount_freq, 
                v.y as f64 * mount_freq, 
                v.z as f64 * mount_freq
            ]) as f32;
            
            // Делаем шум чисто положительным и "острым" (Ridged Noise)
            // 1.0 - abs(n) создает острые пики вместо плавных холмов
            let mountain_noise = raw_mount_noise.abs();

            // --- 5. ФИНАЛЬНАЯ ВЫСОТА ---
            
            // Общая кривизна поверхности (плато, холмы)
            let h_surface = h_base + surface_noise * 0.4;
            
            // Тектонические горы: растут ТОЛЬКО вверх там, где есть stress
            // stress.max(0.0) гарантирует, что впадины не будут "шуметь" как горы
            let h_mountains = stress.max(0.0) * (0.0 + mountain_noise * 2.5);
            
            // Впадины (рифты/желоба): оставляем их более гладкими
            let h_trenches = stress.min(0.0) * mountain_noise * 5.5;

            let final_height = h_surface + h_mountains + h_trenches;

            // Обновляем позицию вершины
            *pos = (v_norm * (world_radius + final_height)).to_array();


            // --- 5. РАСКРАСКА (Biomes) ---
            let color = if final_height < -0.25 {
                LinearRgba::new(0.02, 0.05, 0.2, 1.0) // Глубокий океан
            } else if final_height < -0.05 {
                LinearRgba::new(0.05, 0.2, 0.5, 1.0) // Мелководье
            } else if final_height < 0.02 {
                LinearRgba::new(0.8, 0.7, 0.4, 1.0) // Песок / Пляж
            } else if final_height < 0.15 {
                LinearRgba::new(0.1, 0.4, 0.1, 1.0) // Равнина (Трава)
            } else if final_height < 0.35 {
                LinearRgba::new(0.3, 0.2, 0.15, 1.0) // Предгорья (Земля/Камень)
            } else if final_height < 0.5 {
                LinearRgba::new(0.4, 0.4, 0.4, 1.0) // Высокие скалы
            } else {
                LinearRgba::new(0.9, 0.9, 1.0, 1.0) // Снежные пики
            };

            new_colors.push(color.to_f32_array());
        }

        // Вставляем атрибут цвета в меш
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, new_colors);
    }
    
    // Пересчитываем нормали, чтобы освещение работало правильно на новом рельефе
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
