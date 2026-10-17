mod domains;
mod viewport;
mod world;

use bevy::prelude::*;
use domains::DomainsPlugin;
use viewport::ViewportPlugin;
use world::WorldPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins((
            WorldPlugin,
            DomainsPlugin,
            ViewportPlugin,
        ))
        .run();
}