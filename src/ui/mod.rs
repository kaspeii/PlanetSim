// src/ui/mod.rs

use bevy::prelude::*;
use bevy_egui::{EguiContext, EguiPlugin, EguiPrimaryContextPass, egui};

use crate::{
    domains::{
        climate::climate_ui::draw_climate_tab_system,
        generator::{GenerationRequest, GenerationTask},
        tectonics::{GeologyLayer, tectonics_ui::draw_tectonics_tab_system},
        hydrology::hydrology_ui::draw_hydrology_tab_system,
    },
    viewport::{Globe, MapOverlay, ViewportViewConfig},
    world::{
        pipeline::{PipelineStage, WorldPipeline},
        states::AppState,
    },
};

#[derive(Resource, Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum ActiveTab {
    #[default]
    Tectonics,
    Climate,
    Hydrology,
    Civilization,
}

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }

        app.init_resource::<ActiveTab>()
            .add_systems(
                EguiPrimaryContextPass,
                main_menu_ui.run_if(in_state(AppState::MainMenu)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                (render_ui_shell, generation_progress_overlay)
                    .chain().run_if(in_state(AppState::InGame)),
            )
            .add_systems(Update, manage_scene_visibility);
    }
}

fn manage_scene_visibility(
    state: Res<State<AppState>>,
    geology: Res<GeologyLayer>,
    mut q_globe: Query<&mut Visibility, With<Globe>>,
) {
    let should_show = *state.get() == AppState::InGame && !geology.elevations.is_empty();
    let target_vis = if should_show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    for mut vis in q_globe.iter_mut() {
        if *vis != target_vis {
            *vis = target_vis;
        }
    }
}

fn main_menu_ui(
    mut contexts: Query<&mut EguiContext>,
    mut next_state: ResMut<NextState<AppState>>,
    mut gen_req: ResMut<GenerationRequest>,
) -> Result {
    let Ok(mut egui_ctx) = contexts.single_mut() else {
        return Ok(());
    };

    egui::Window::new("main_menu")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(egui_ctx.get_mut(), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                ui.heading("Planet Simulator");
                ui.add_space(20.0);

                let btn = ui.add_sized([220.0, 50.0], egui::Button::new("Сгенерировать планету"));
                if btn.clicked() {
                    gen_req.request(PipelineStage::Tectonics);
                    next_state.set(AppState::InGame);
                }
                ui.add_space(10.0);
            });
        });

    Ok(())
}

/// Неблокирующий виджет лога генерации (показывает историю этапов, тайминги и прогресс-бар)
fn generation_progress_overlay(
    mut contexts: Query<&mut EguiContext>,
    tasks: Query<&GenerationTask>,
) -> Result {
    let Some(task) = tasks.iter().next() else {
        return Ok(());
    };
    let Ok(mut egui_ctx) = contexts.single_mut() else {
        return Ok(());
    };

    let snap = task.logger.snapshot();
    let progress = snap.progress();

    // Если это первая генерация из меню — окно по центру,
    // если перегенерируем на лету — аккуратная панель сверху по центру
    let (align, offset) = if task.is_initial {
        (egui::Align2::CENTER_CENTER, [0.0, 0.0])
    } else {
        (egui::Align2::CENTER_TOP, [0.0, 15.0])
    };

    let content_width = 440.0;

    egui::Window::new("generation_status")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(align, offset)
        .fixed_size([content_width, 0.0])
        .show(egui_ctx.get_mut(), |ui| {
            ui.set_width(content_width);
            ui.add_space(4.0);

            // === Заголовок окна ===
            ui.horizontal(|ui| {
                ui.set_width(content_width);
                if snap.is_finished {
                    ui.label(
                        egui::RichText::new("✔")
                            .color(egui::Color32::from_rgb(80, 210, 120))
                            .strong(),
                    );
                    ui.strong("Генерация мира завершена!");
                } else {
                    ui.spinner();
                    ui.strong("Генерация мира...");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "Шаг {}/{}  •  {:.2} с",
                            snap.completed_steps.min(snap.total_steps),
                            snap.total_steps,
                            snap.total_elapsed_ms as f32 / 1000.0
                        ))
                        .weak()
                        .monospace(),
                    );
                });
            });

            ui.separator();
            ui.add_space(2.0);

            // === Список этапов с таймингами ===
            for entry in &snap.entries {
                ui.horizontal(|ui| {
                    ui.set_width(content_width);
                    if let Some(ms) = entry.duration_ms {
                        // Завершенный этап
                        ui.label(
                            egui::RichText::new("✔")
                                .color(egui::Color32::from_rgb(80, 210, 120)),
                        );
                        ui.label(
                            egui::RichText::new(&entry.message)
                                .color(egui::Color32::from_gray(200)),
                        );
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(format!("{} мс", ms))
                                        .weak()
                                        .monospace(),
                                );
                            },
                        );
                    } else {
                        // Текущий выполняющийся этап
                        let live_ms = entry.started_at.elapsed().as_millis();
                        ui.spinner();
                        ui.label(
                            egui::RichText::new(&entry.message)
                                .strong()
                                .color(egui::Color32::WHITE),
                        );
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(format!("{} мс...", live_ms))
                                        .color(egui::Color32::from_rgb(240, 200, 80))
                                        .monospace(),
                                );
                            },
                        );
                    }
                });
                ui.add_space(2.0);
            }

            ui.add_space(6.0);
            ui.add(
                egui::ProgressBar::new(progress)
                    .show_percentage()
                    .desired_width(content_width),
            );
            ui.add_space(4.0);
        });

    Ok(())
}

/// Эксклюзивная система UI: сама достает контекст egui 0.35, строит Area и Panel,
/// а отрисовку правой вкладки делегирует доменам через run_system_cached_with,
/// не зная ничего про их внутренние конфиги!
fn render_ui_shell(world: &mut World) -> Result {
    let mut contexts = world.query::<&mut EguiContext>();
    let Ok(mut egui_ctx) = contexts.single_mut(world) else {
        return Ok(());
    };
    let ctx = egui_ctx.get_mut().clone();

    let rect = ctx.content_rect();

    egui::Area::new(egui::Id::new("root_ui_shell"))
        .fixed_pos(rect.min)
        .order(egui::Order::Background)
        .show(&ctx, |ui| {
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());

            // === 1. НИЖНЯЯ ПАНЕЛЬ (Вкладки / Taskbar) ===
            egui::Panel::bottom("bottom_panel")
                .exact_size(40.0)
                .show(ui, |ui| {
                    let mut active_tab = world.resource_mut::<ActiveTab>();
                    ui.horizontal_centered(|ui| {
                        ui.selectable_value(&mut *active_tab, ActiveTab::Tectonics, "Тектоника");
                        ui.selectable_value(&mut *active_tab, ActiveTab::Climate, "Климат");
                        ui.selectable_value(&mut *active_tab, ActiveTab::Hydrology, "Гидрология");
                        ui.selectable_value(
                            &mut *active_tab,
                            ActiveTab::Civilization,
                            "Цивилизации",
                        );
                    });
                });

            // === 2. ЛЕВАЯ ПАНЕЛЬ (Режимы отображения) ===
            egui::Panel::left("left_panel")
                .default_size(200.0)
                .show(ui, |ui| {
                    ui.heading("Отображение");
                    ui.separator();

                    let mut recolor = false;
                    let mut reposition = false;

                    ui.label("Слои карты:");

                    world.resource_scope(|_world, mut overlay: Mut<MapOverlay>| {
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::SpaceView, "Базовый").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Elevation, "Рельеф").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::TectonicPlates, "Тектонические плиты").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Lithology, "Породы").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Temperature, "Температура").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Humidity, "Влажность").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Precipitation, "Осадки").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Albedo, "Альбедо").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Continentality, "Континентальность").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Pressure, "Давление").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Winds, "Ветра").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Soils, "Почвы").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::SoilMoisture, "Влажность почвы").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Biomes, "Биомы").changed();
                    });

                    ui.separator();
                    ui.label("Модификаторы:");
                    
                    world.resource_scope(|_world, mut view_cfg: Mut<ViewportViewConfig>| {
                        if ui.checkbox(&mut view_cfg.flat_water, "Плоский океан").changed() {
                            reposition = true;
                        }
                        ui.checkbox(&mut view_cfg.show_axis_equator, "Ось и Экватор");
                        ui.checkbox(&mut view_cfg.show_rivers, "3D-русла рек");
                        ui.checkbox(&mut view_cfg.show_wind_arrows, "3D-стрелки ветров");
                        ui.checkbox(&mut view_cfg.show_ocean_currents, "3D-стрелки течений");
                    });

                    if reposition || recolor {
                        let mut pipeline = world.resource_mut::<WorldPipeline>();
                        if reposition {
                            pipeline.reposition_mesh = true;
                        }
                        if recolor {
                            pipeline.recolor_mesh = true;
                        }
                    }
                });

            // === 3. ПРАВАЯ ПАНЕЛЬ (Делегируем отрисовку доменам!) ===
            let current_tab = *world.resource::<ActiveTab>();

            egui::Panel::right("right_panel")
                .default_size(300.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| match current_tab {
                        ActiveTab::Tectonics => {
                            let _ = world.run_system_cached_with(draw_tectonics_tab_system, ui);
                        }
                        ActiveTab::Climate => {
                            let _ = world.run_system_cached_with(draw_climate_tab_system, ui);
                        }
                        ActiveTab::Hydrology => {
                            let _ = world.run_system_cached_with(draw_hydrology_tab_system, ui);
                        }
                        ActiveTab::Civilization => {
                            ui.label("Настройки цивилизаций (в разработке)");
                        }
                    });
                });
        });

    Ok(())
}
