use bevy::log::LogPlugin;
use bevy::prelude::*;

mod events;
mod prolog_wrapper;
mod ui;
use crate::events::{GeneratePreviewRequest, PreviewGenerated};
use crate::prolog_wrapper::PrologPlugin;
use crate::ui::UiPlugin;
use bevy_async_ecs::AsyncEcsPlugin;

pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(LogPlugin {
            filter: "info,bevy=info,wgpu=warn,daccad=trace".into(),
            ..default()
        }))
        .add_plugins(AsyncEcsPlugin)
        .add_event::<GeneratePreviewRequest>()
        .add_event::<PreviewGenerated>()
        .add_plugins((UiPlugin, PrologPlugin))
        .run();
}
