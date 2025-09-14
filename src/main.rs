use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

use crate::ui::setup::*;
use crate::ui::update::*;
mod ui;
mod prolog;

pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(LogPlugin {
            filter: "info,bevy=info,wgpu=warn,daccad=trace".into(),
            ..default()
        }))
        .add_plugins(bevy_async_ecs::AsyncEcsPlugin)
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, (setup, init_async_world))
        .add_systems(EguiPrimaryContextPass, setup_fonts.run_if(run_once))
        .add_systems(EguiPrimaryContextPass, egui_ui)
        .add_systems(Update, update_preview_transforms)
        .run();
}
