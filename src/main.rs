use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

use crate::ui::setup::*;
use crate::ui::update::*;
mod ui;

pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(LogPlugin {
            // Show our crate logs too (by default Bevy filters to core crates)
            filter: "info,bevy=info,wgpu=warn,daccad=trace".into(),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(EguiPrimaryContextPass, (setup_fonts, egui_ui))
        .run();
}
