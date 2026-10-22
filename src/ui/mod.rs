// /src/ui/mod.rs

use bevy::prelude::*;
use bevy_egui::{EguiContext, EguiPlugin, EguiPrimaryContextPass, egui};

use crate::{
    domains::{
        climate::climate_ui::draw_climate_tab_system,
        generator::is_generating,
        tectonics::tectonics_ui::draw_tectonics_tab_system,
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
                loading_screen_ui.run_if(is_generating),
            )
            .add_systems(
                EguiPrimaryContextPass,
                render_ui_shell.run_if(in_state(AppState::InGame)),
            )
            .add_systems(Update, manage_scene_visibility);
    }
}

fn manage_scene_visibility(
    state: Res<State<AppState>>,
    mut q_globe: Query<&mut Visibility, With<Globe>>,
) {
    if state.is_changed() {
        let new_vis = match state.get() {
            AppState::InGame => Visibility::Inherited,
            _ => Visibility::Hidden,
        };
        for mut vis in q_globe.iter_mut() {
            *vis = new_vis;
        }
    }
}

fn main_menu_ui(
    mut contexts: Query<&mut EguiContext>,
    mut next_state: ResMut<NextState<AppState>>,
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
                    next_state.set(AppState::Generating(PipelineStage::Tectonics));
                }
                ui.add_space(10.0);
            });
        });

    Ok(())
}

fn loading_screen_ui(
    mut contexts: Query<&mut EguiContext>,
    state: Res<State<AppState>>,
) -> Result {
    let Ok(mut egui_ctx) = contexts.single_mut() else {
        return Ok(());
    };

    let text = match state.get() {
        AppState::Generating(PipelineStage::Tectonics) => {
            "Формирование тектонических плит и климата..."
        }
        AppState::Generating(PipelineStage::Climate) => {
            "Пересчет атмосферы и океанических течений..."
        }
        _ => "Загрузка...",
    };

    egui::Window::new("loading_screen")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(egui_ctx.get_mut(), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(15.0);
                ui.heading(text);
                ui.add_space(15.0);
                ui.spinner();
                ui.add_space(15.0);
            });
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
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Temperature, "Температура").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Albedo, "Альбедо").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Continentality, "Континентальность").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Pressure, "Давление").changed();
                        recolor |= ui.radio_value(&mut *overlay, MapOverlay::Winds, "Ветра").changed();
                    });

                    ui.separator();
                    ui.label("Модификаторы:");
                    
                    world.resource_scope(|_world, mut view_cfg: Mut<ViewportViewConfig>| {
                        if ui.checkbox(&mut view_cfg.flat_water, "Плоский океан").changed() {
                            reposition = true;
                        }
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
                            ui.label("Настройки рек (в разработке)");
                        }
                        ActiveTab::Civilization => {
                            ui.label("Настройки цивилизаций (в разработке)");
                        }
                    });
                });
        });

    Ok(())
}