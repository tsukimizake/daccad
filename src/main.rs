use bevy::log::LogPlugin;
use bevy::prelude::*;

mod cadhr_lang_wrapper;
mod events;
mod ui;
use crate::cadhr_lang_wrapper::CadhrLangPlugin;
use crate::events::{
    CadhrLangOutput, CollisionPreviewGenerated, GenerateCollisionPreviewRequest,
    GeneratePreviewRequest, PreviewGenerated,
};
use crate::ui::UiPlugin;
pub fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(LogPlugin {
                    filter: "info,bevy=info,wgpu=warn,cadhr=trace".into(),
                    ..default()
                })
                .set(bevy::window::WindowPlugin {
                    close_when_requested: false,
                    ..default()
                }),
        )
        .add_message::<GeneratePreviewRequest>()
        .add_message::<PreviewGenerated>()
        .add_message::<GenerateCollisionPreviewRequest>()
        .add_message::<CollisionPreviewGenerated>()
        .add_message::<CadhrLangOutput>()
        .add_plugins((UiPlugin, CadhrLangPlugin))
        .run();
}
