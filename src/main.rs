use bevy::log::LogPlugin;
use bevy::prelude::*;

mod cadhr_lang_wrapper;
mod events;
mod ui;
use crate::cadhr_lang_wrapper::CadhrLangPlugin;
use crate::events::{CadhrLangOutput, GeneratePreviewRequest, PreviewGenerated};
use crate::ui::UiPlugin;
use bevy_async_ecs::AsyncEcsPlugin;

pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(LogPlugin {
            filter: "info,bevy=info,wgpu=warn,cadhr=trace".into(),
            ..default()
        }))
        .add_plugins(AsyncEcsPlugin)
        .add_message::<GeneratePreviewRequest>()
        .add_message::<PreviewGenerated>()
        .add_message::<CadhrLangOutput>()
        .add_plugins((UiPlugin, CadhrLangPlugin))
        .run();
}
