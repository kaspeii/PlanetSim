// src/ui/mod.rs

mod main_menu;
mod overlays;
mod shell;

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

use crate::{domains::tectonics::GeologyLayer, viewport::Globe, world::states::AppState};

use main_menu::main_menu_ui;
use overlays::{
    LegendState, generation_progress_overlay, map_legend_overlay, save_load_progress_overlay,
};
use shell::{render_ui_shell, ShellLayout};

#[derive(Resource, Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum ActiveTab {
    #[default]
    World,
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
            .init_resource::<LegendState>()
            .init_resource::<ShellLayout>() 
            .add_systems(
                EguiPrimaryContextPass,
                (
                    render_ui_shell,
                    main_menu_ui.run_if(in_state(AppState::MainMenu)),
                    generation_progress_overlay,
                    save_load_progress_overlay,
                    map_legend_overlay.run_if(in_state(AppState::InGame)),
                )
                    .chain(),
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
