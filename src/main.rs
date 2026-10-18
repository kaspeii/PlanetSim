mod domains;
mod ui;
mod viewport;
mod world;

use bevy::prelude::*;
use domains::DomainsPlugin;
use ui::UiPlugin;
use viewport::ViewportPlugin;
use world::WorldPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins((WorldPlugin, DomainsPlugin, ViewportPlugin, UiPlugin))
        .run();
}
