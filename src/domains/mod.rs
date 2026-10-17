pub mod tectonics;
// Будущие подсистемы:
// pub mod climate;
// pub mod hydrology;
// pub mod civilization;

use bevy::prelude::*;
use tectonics::TectonicsPlugin;

pub struct DomainsPlugin;

impl Plugin for DomainsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TectonicsPlugin);
    }
}