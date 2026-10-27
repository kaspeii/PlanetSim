// src/domains/save.rs

use std::{
    fs,
    path::{Path, PathBuf},
};
use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};

use crate::{
    domains::{
        climate::{ClimateConfig, climate_data::ClimateLayer},
        generator::GenerationRequest,
        hydrology::{HydrologyConfig, HydrologyLayer},
        tectonics::{GeologyLayer, TectonicsConfig},
    },
    world::{
        WorldConfig,
        pipeline::{PipelineStage, WorldPipeline},
        states::AppState,
        topology::WorldTopology,
    },
};

const PLANET_SAVE_DIR: &str = "saves/planets";
const CONFIG_SAVE_DIR: &str = "saves/configs";
const PLANET_FILE_VERSION: u32 = 1;

/// Бинарный контейнер сохранения планеты (НЕ хранит конфиги генерации).
/// Каждый слой упакован в отдельный чанк байтов, чтобы отсутствие или устаревание
/// одного этапа не ломало чтение предыдущих этапов.
#[derive(Serialize, Deserialize)]
pub struct PlanetSaveFile {
    pub version: u32,
    pub radius: f32,
    pub subdivisions: u32,
    pub geology_bytes: Option<Vec<u8>>,
    pub climate_bytes: Option<Vec<u8>>,
    pub hydrology_bytes: Option<Vec<u8>>,
}

/// Текстовый JSON-пресет настроек генератора (отдельно от планет)
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ConfigPresetFile {
    pub tectonics: TectonicsConfig,
    pub climate: ClimateConfig,
    pub hydrology: HydrologyConfig,
}

#[derive(Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub size_kb: u64,
}

#[derive(Clone)]
pub struct MissingStagePrompt {
    pub planet_name: String,
    pub missing_from: PipelineStage,
    pub reason: String,
}

#[derive(Resource)]
pub struct SaveUiState {
    pub show_planets_window: bool,
    pub show_configs_window: bool,

    pub planet_save_name: String,
    pub save_include_climate: bool,
    pub save_include_hydrology: bool,
    pub auto_regenerate_missing: bool,

    pub config_save_name: String,
    pub load_tectonics_cfg: bool,
    pub load_climate_cfg: bool,
    pub load_hydrology_cfg: bool,

    pub planet_files: Vec<FileEntry>,
    pub config_files: Vec<FileEntry>,

    pub status_message: Option<(String, bool)>, // (текст, is_error)
    pub pending_prompt: Option<MissingStagePrompt>,
}

impl Default for SaveUiState {
    fn default() -> Self {
        let mut state = Self {
            show_planets_window: false,
            show_configs_window: false,
            planet_save_name: "planet_01".to_string(),
            save_include_climate: true,
            save_include_hydrology: true,
            auto_regenerate_missing: false,
            config_save_name: "preset_default".to_string(),
            load_tectonics_cfg: true,
            load_climate_cfg: true,
            load_hydrology_cfg: true,
            planet_files: Vec::new(),
            config_files: Vec::new(),
            status_message: None,
            pending_prompt: None,
        };
        state.refresh_lists();
        state
    }
}

impl SaveUiState {
    pub fn refresh_lists(&mut self) {
        self.planet_files = scan_dir(PLANET_SAVE_DIR, "planet");
        self.config_files = scan_dir(CONFIG_SAVE_DIR, "json");
    }

    fn set_ok(&mut self, msg: impl Into<String>) {
        let s = msg.into();
        info!("{}", s);
        self.status_message = Some((s, false));
    }

    fn set_err(&mut self, msg: impl Into<String>) {
        let s = msg.into();
        warn!("{}", s);
        self.status_message = Some((s, true));
    }
}

fn scan_dir(dir: &str, ext: &str) -> Vec<FileEntry> {
    let _ = fs::create_dir_all(dir);
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some(ext) {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unnamed")
                    .to_string();
                let size_kb = entry.metadata().map(|m| m.len() / 1024).unwrap_or(0);
                out.push(FileEntry {
                    name,
                    path,
                    size_kb,
                });
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn sanitize_filename(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "unnamed".to_string();
    }
    trimmed
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn stage_label(stage: PipelineStage) -> &'static str {
    match stage {
        PipelineStage::Tectonics => "Тектоника (с нуля)",
        PipelineStage::Climate => "Климат -> Гидрология",
        PipelineStage::Hydrology => "Гидрология и Биомы",
    }
}

/// Система отрисовки модальных окон сохранения/загрузки планет, конфигов и диалога догенерации
pub fn draw_save_load_windows_system(
    In(ctx): In<egui::Context>,
    mut save_ui: ResMut<SaveUiState>,
    mut world_config: ResMut<WorldConfig>,
    mut topology: ResMut<WorldTopology>,
    mut geology: ResMut<GeologyLayer>,
    mut climate: ResMut<ClimateLayer>,
    mut hydrology: ResMut<HydrologyLayer>,
    mut t_config: ResMut<TectonicsConfig>,
    mut c_config: ResMut<ClimateConfig>,
    mut h_config: ResMut<HydrologyConfig>,
    mut pipeline: ResMut<WorldPipeline>,
    mut gen_req: ResMut<GenerationRequest>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // === 1. ОКНО СОХРАНЕНИЯ / ЗАГРУЗКИ ПЛАНЕТ ===
    if save_ui.show_planets_window {
        let mut open = save_ui.show_planets_window;
        egui::Window::new("💾 Сохранения планет")
            .open(&mut open)
            .resizable(true)
            .default_width(420.0)
            .show(&ctx, |ui| {
                // --- Секция сохранения текущей планеты ---
                ui.heading("Сохранить текущую планету");
                let has_planet = !geology.elevations.is_empty();

                ui.horizontal(|ui| {
                    ui.label("Имя файла:");
                    ui.text_edit_singleline(&mut save_ui.planet_save_name);
                });

                ui.horizontal(|ui| {
                    ui.label("Включить этапы:");
                    let mut tect_always = true;
                    ui.add_enabled(false, egui::Checkbox::new(&mut tect_always, "Тектоника"));
                    ui.checkbox(&mut save_ui.save_include_climate, "Климат");
                    if !save_ui.save_include_climate {
                        save_ui.save_include_hydrology = false;
                    }
                    ui.add_enabled(
                        save_ui.save_include_climate,
                        egui::Checkbox::new(&mut save_ui.save_include_hydrology, "Гидрология"),
                    );
                });

                ui.add_space(4.0);
                ui.add_enabled_ui(has_planet, |ui| {
                    if ui
                        .add_sized(
                            [ui.available_width(), 32.0],
                            egui::Button::new("💾 Сохранить планету (.planet)"),
                        )
                        .clicked()
                    {
                        let clean_name = sanitize_filename(&save_ui.planet_save_name);
                        let _ = fs::create_dir_all(PLANET_SAVE_DIR);
                        let path =
                            Path::new(PLANET_SAVE_DIR).join(format!("{}.planet", clean_name));

                        let geo_bytes = bincode::serialize(&*geology).ok();
                        let clim_bytes = if save_ui.save_include_climate
                            && !climate.temperatures.is_empty()
                        {
                            bincode::serialize(&*climate).ok()
                        } else {
                            None
                        };
                        let hydro_bytes = if save_ui.save_include_hydrology
                            && !hydrology.biomes.is_empty()
                        {
                            bincode::serialize(&*hydrology).ok()
                        } else {
                            None
                        };

                        let save_data = PlanetSaveFile {
                            version: PLANET_FILE_VERSION,
                            radius: world_config.radius,
                            subdivisions: topology.subdivisions,
                            geology_bytes: geo_bytes,
                            climate_bytes: clim_bytes,
                            hydrology_bytes: hydro_bytes,
                        };

                        match bincode::serialize(&save_data)
                            .map_err(|e| e.to_string())
                            .and_then(|bytes| fs::write(&path, bytes).map_err(|e| e.to_string()))
                        {
                            Ok(()) => {
                                save_ui.refresh_lists();
                                save_ui.set_ok(format!("Планета «{}» успешно сохранена", clean_name));
                            }
                            Err(err) => {
                                save_ui.set_err(format!("Ошибка сохранения планеты: {}", err));
                            }
                        }
                    }
                });
                if !has_planet {
                    ui.small("Сначала сгенерируйте или загрузите планету, чтобы сохранить её.");
                }

                ui.separator();

                // --- Секция загрузки планет ---
                ui.horizontal(|ui| {
                    ui.heading("Доступные планеты");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("🔄 Обновить").clicked() {
                            save_ui.refresh_lists();
                        }
                    });
                });

                ui.checkbox(
                    &mut save_ui.auto_regenerate_missing,
                    "Автоматически догенерировать недостающие этапы без вопроса",
                );
                ui.add_space(4.0);

                let files = save_ui.planet_files.clone();
                if files.is_empty() {
                    ui.weak("Нет сохраненных планет в папке saves/planets/");
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(240.0)
                        .show(ui, |ui| {
                            for entry in files {
                                ui.horizontal(|ui| {
                                    ui.strong(&entry.name);
                                    ui.weak(format!("({} КБ)", entry.size_kb));

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button("🗑").on_hover_text("Удалить").clicked() {
                                                let _ = fs::remove_file(&entry.path);
                                                save_ui.refresh_lists();
                                            }
                                            if ui.button("📂 Загрузить").clicked() {
                                                load_planet_from_path(
                                                    &entry.path,
                                                    &entry.name,
                                                    &mut save_ui,
                                                    &mut world_config,
                                                    &mut topology,
                                                    &mut geology,
                                                    &mut climate,
                                                    &mut hydrology,
                                                    &mut pipeline,
                                                    &mut gen_req,
                                                    &mut next_state,
                                                );
                                            }
                                        },
                                    );
                                });
                                ui.separator();
                            }
                        });
                }

                if let Some((ref msg, is_err)) = save_ui.status_message {
                    ui.add_space(4.0);
                    let color = if is_err {
                        egui::Color32::from_rgb(230, 90, 90)
                    } else {
                        egui::Color32::from_rgb(90, 210, 120)
                    };
                    ui.colored_label(color, msg);
                }
            });
        save_ui.show_planets_window = open;
    }

    // === 2. ОКНО ПРЕСЕТОВ КОНФИГОВ (ОТДЕЛЬНО ОТ ПЛАНЕТ) ===
    if save_ui.show_configs_window {
        let mut open = save_ui.show_configs_window;
        egui::Window::new("⚙ Пресеты конфигов генерации")
            .open(&mut open)
            .resizable(true)
            .default_width(400.0)
            .show(&ctx, |ui| {
                ui.heading("Сохранить текущие настройки");
                ui.horizontal(|ui| {
                    ui.label("Имя пресета:");
                    ui.text_edit_singleline(&mut save_ui.config_save_name);
                });

                if ui
                    .add_sized(
                        [ui.available_width(), 30.0],
                        egui::Button::new("💾 Сохранить пресет (.json)"),
                    )
                    .clicked()
                {
                    let clean_name = sanitize_filename(&save_ui.config_save_name);
                    let _ = fs::create_dir_all(CONFIG_SAVE_DIR);
                    let path = Path::new(CONFIG_SAVE_DIR).join(format!("{}.json", clean_name));

                    let preset = ConfigPresetFile {
                        tectonics: (*t_config).clone(),
                        climate: (*c_config).clone(),
                        hydrology: (*h_config).clone(),
                    };

                    match serde_json::to_string_pretty(&preset)
                        .map_err(|e| e.to_string())
                        .and_then(|json| fs::write(&path, json).map_err(|e| e.to_string()))
                    {
                        Ok(()) => {
                            save_ui.refresh_lists();
                            save_ui.set_ok(format!("Пресет «{}» сохранен", clean_name));
                        }
                        Err(err) => {
                            save_ui.set_err(format!("Ошибка сохранения пресета: {}", err));
                        }
                    }
                }

                ui.add_space(4.0);
                if ui
                    .button("↺ Сбросить все конфиги к значениям по умолчанию")
                    .clicked()
                {
                    *t_config = TectonicsConfig::default();
                    *c_config = ClimateConfig::default();
                    *h_config = HydrologyConfig::default();
                    save_ui.set_ok("Все конфиги сброшены к значениям по умолчанию");
                }

                ui.separator();
                ui.heading("Загрузить пресет");
                ui.horizontal(|ui| {
                    ui.label("Применять к:");
                    ui.checkbox(&mut save_ui.load_tectonics_cfg, "Тектоника");
                    ui.checkbox(&mut save_ui.load_climate_cfg, "Климат");
                    ui.checkbox(&mut save_ui.load_hydrology_cfg, "Гидрология");
                });

                let files = save_ui.config_files.clone();
                if files.is_empty() {
                    ui.weak("Нет сохраненных пресетов в папке saves/configs/");
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .show(ui, |ui| {
                            for entry in files {
                                ui.horizontal(|ui| {
                                    ui.strong(&entry.name);
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button("🗑").clicked() {
                                                let _ = fs::remove_file(&entry.path);
                                                save_ui.refresh_lists();
                                            }
                                            if ui.button("📥 Применить").clicked() {
                                                match fs::read_to_string(&entry.path)
                                                    .map_err(|e| e.to_string())
                                                    .and_then(|s| {
                                                        serde_json::from_str::<ConfigPresetFile>(&s)
                                                            .map_err(|e| e.to_string())
                                                    }) {
                                                    Ok(preset) => {
                                                        if save_ui.load_tectonics_cfg {
                                                            *t_config = preset.tectonics;
                                                        }
                                                        if save_ui.load_climate_cfg {
                                                            *c_config = preset.climate;
                                                        }
                                                        if save_ui.load_hydrology_cfg {
                                                            *h_config = preset.hydrology;
                                                        }
                                                        save_ui.set_ok(format!(
                                                            "Пресет «{}» применен",
                                                            entry.name
                                                        ));
                                                    }
                                                    Err(err) => {
                                                        save_ui.set_err(format!(
                                                            "Ошибка чтения пресета: {}",
                                                            err
                                                        ));
                                                    }
                                                }
                                            }
                                        },
                                    );
                                });
                                ui.separator();
                            }
                        });
                }
            });
        save_ui.show_configs_window = open;
    }

    // === 3. МОДАЛЬНОЕ ОКНО ДОГЕНЕРАЦИИ НЕДОСТАЮЩИХ ЭТАПОВ ===
    if let Some(prompt) = save_ui.pending_prompt.clone() {
        egui::Window::new("⚠ В сохранении отсутствуют этапы")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([420.0, 0.0])
            .show(&ctx, |ui| {
                ui.label(format!(
                    "Планета «{}» загружена частично: {}.",
                    prompt.planet_name, prompt.reason
                ));
                ui.add_space(6.0);
                ui.strong(format!(
                    "Догенерировать недостающие стадии начиная с этапа «{}»?",
                    stage_label(prompt.missing_from)
                ));
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui
                        .button(format!(
                            "▶ Догенерировать ({})",
                            stage_label(prompt.missing_from)
                        ))
                        .clicked()
                    {
                        gen_req.request(prompt.missing_from);
                        save_ui.pending_prompt = None;
                    }

                    if ui.button("Оставить как есть").clicked() {
                        save_ui.pending_prompt = None;
                    }
                });
            });
    }
}

/// Умный загрузчик планеты: проверяет сетку, поочередно распаковывает слои
/// и при нехватке любого этапа инициирует/предлагает догенерацию ровно с него.
fn load_planet_from_path(
    path: &Path,
    planet_name: &str,
    save_ui: &mut SaveUiState,
    world_config: &mut WorldConfig,
    topology: &mut WorldTopology,
    geology: &mut GeologyLayer,
    climate: &mut ClimateLayer,
    hydrology: &mut HydrologyLayer,
    pipeline: &mut WorldPipeline,
    gen_req: &mut GenerationRequest,
    next_state: &mut NextState<AppState>,
) {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            save_ui.set_err(format!("Не удалось прочитать файл: {}", e));
            return;
        }
    };

    let save_file: PlanetSaveFile = match bincode::deserialize(&bytes) {
        Ok(s) => s,
        Err(e) => {
            save_ui.set_err(format!("Поврежденный заголовок сохранения: {}", e));
            return;
        }
    };

    // 1. Синхронизируем топологию сферы, если планета была сохранена с другим subdivisions
    world_config.radius = save_file.radius;
    if topology.subdivisions != save_file.subdivisions {
        info!(
            "Перестроение топологии сетки под сохранение: subdivisions {} -> {}",
            topology.subdivisions, save_file.subdivisions
        );
        *topology = WorldTopology::new(save_file.subdivisions);
    }
    let num_cells = topology.cells.len();

    let mut missing_stage: Option<(PipelineStage, String)> = None;

    // 2. Пытаемся загрузить Тектонику (GeologyLayer)
    match save_file
        .geology_bytes
        .as_deref()
        .map(bincode::deserialize::<GeologyLayer>)
    {
        Some(Ok(loaded_geo)) if loaded_geo.elevations.len() == num_cells => {
            *geology = loaded_geo;
        }
        Some(Ok(_)) => {
            geology.resize(num_cells);
            missing_stage = Some((
                PipelineStage::Tectonics,
                "размер сетки рельефа не совпал с топологией".to_string(),
            ));
        }
        Some(Err(_)) => {
            geology.resize(num_cells);
            missing_stage = Some((
                PipelineStage::Tectonics,
                "формат слоя тектоники устарел".to_string(),
            ));
        }
        None => {
            geology.resize(num_cells);
            missing_stage = Some((
                PipelineStage::Tectonics,
                "отсутствует слой тектоники".to_string(),
            ));
        }
    }

    // 3. Пытаемся загрузить Климат (ClimateLayer), если Тектоника успешна
    if missing_stage.is_none() {
        match save_file
            .climate_bytes
            .as_deref()
            .map(bincode::deserialize::<ClimateLayer>)
        {
            Some(Ok(loaded_clim)) if loaded_clim.temperatures.len() == num_cells => {
                *climate = loaded_clim;
            }
            Some(Ok(_)) | Some(Err(_)) => {
                *climate = ClimateLayer::default();
                climate.resize(num_cells);
                missing_stage = Some((
                    PipelineStage::Climate,
                    "данные климата устарели или не совпадают по сетке".to_string(),
                ));
            }
            None => {
                *climate = ClimateLayer::default();
                climate.resize(num_cells);
                missing_stage = Some((
                    PipelineStage::Climate,
                    "отсутствует этап климата".to_string(),
                ));
            }
        }
    } else {
        *climate = ClimateLayer::default();
        climate.resize(num_cells);
    }

    // 4. Пытаемся загрузить Гидрологию (HydrologyLayer), если Тектоника и Климат успешны
    if missing_stage.is_none() {
        match save_file
            .hydrology_bytes
            .as_deref()
            .map(bincode::deserialize::<HydrologyLayer>)
        {
            Some(Ok(loaded_hydro)) if loaded_hydro.biomes.len() == num_cells => {
                *hydrology = loaded_hydro;
            }
            Some(Ok(_)) | Some(Err(_)) => {
                *hydrology = HydrologyLayer::default();
                hydrology.resize(num_cells);
                missing_stage = Some((
                    PipelineStage::Hydrology,
                    "данные гидрологии и биомов устарели".to_string(),
                ));
            }
            None => {
                *hydrology = HydrologyLayer::default();
                hydrology.resize(num_cells);
                missing_stage = Some((
                    PipelineStage::Hydrology,
                    "отсутствует этап гидрологии и биомов".to_string(),
                ));
            }
        }
    } else {
        *hydrology = HydrologyLayer::default();
        hydrology.resize(num_cells);
    }

    // Перестраиваем 3D-глобус и переходим в InGame
    pipeline.rebuild_mesh = true;
    pipeline.rebuild_rivers = true;
    next_state.set(AppState::InGame);
    save_ui.planet_save_name = planet_name.to_string();

    // 5. Если какого-то этапа не хватило — либо сразу догенерируем, либо спрашиваем пользователя
    if let Some((stage, reason)) = missing_stage {
        if save_ui.auto_regenerate_missing {
            save_ui.set_ok(format!(
                "Загружена «{}» ({}). Запущена авто-догенерация...",
                planet_name, reason
            ));
            gen_req.request(stage);
        } else {
            save_ui.set_ok(format!("Планета «{}» загружена частично ({})", planet_name, reason));
            save_ui.pending_prompt = Some(MissingStagePrompt {
                planet_name: planet_name.to_string(),
                missing_from: stage,
                reason,
            });
        }
    } else {
        save_ui.set_ok(format!("Планета «{}» полностью загружена!", planet_name));
    }
}