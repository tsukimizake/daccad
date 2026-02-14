use bevy::prelude::*;
use bevy_file_dialog::prelude::*;
use derived_deref::{Deref, DerefMut};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod setup;
pub mod update;

pub struct UiPlugin;

pub struct ThreeMfFileContents;

pub struct SessionSaveContents;
pub struct SessionLoadContents;

#[derive(Serialize, Deserialize, Clone)]
pub struct PreviewState {
    pub query: String,
    pub zoom: f32,
    pub rotate_x: f64,
    pub rotate_y: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SessionPreviews {
    pub previews: Vec<PreviewState>,
}

/// Preview target data stored as a component on the root entity.
/// When this entity is despawned, all children are automatically removed.
#[derive(Component, Clone)]
pub struct PreviewTarget {
    pub mesh_handle: Handle<Mesh>,
    pub rt_image: Handle<Image>,
    pub rt_size: UVec2,
    pub camera_entity: Entity,
    pub base_camera_distance: f32,
    pub zoom: f32,
    pub rotate_x: f64,
    pub rotate_y: f64,
    pub query: String,
}

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
        use setup::*;
        use update::{
            egui_ui, handle_cadhr_lang_output, on_preview_generated, session_loaded, session_saved,
            threemf_saved, update_preview_transforms,
        };

        app.add_plugins(EguiPlugin::default())
            .add_plugins(
                FileDialogPlugin::new()
                    .with_save_file::<SessionSaveContents>()
                    .with_pick_directory::<SessionLoadContents>()
                    .with_save_file::<ThreeMfFileContents>(),
            )
            .add_systems(Startup, setup)
            .add_systems(EguiPrimaryContextPass, setup_fonts.run_if(run_once))
            .add_systems(EguiPrimaryContextPass, egui_ui)
            .add_systems(
                Update,
                (
                    on_preview_generated,
                    update_preview_transforms,
                    handle_cadhr_lang_output,
                ),
            )
            .add_systems(Update, (session_saved, session_loaded, threemf_saved))
            .insert_resource(EditorText("main :- cube(10, 20, 30).".to_string()))
            .insert_resource(NextRequestId::default())
            .insert_resource(ErrorMessage::default())
            .insert_resource(CurrentFilePath::default())
            .insert_resource(PendingPreviewStates::default());
    }
}

#[derive(Resource, Default, Clone, Deref, DerefMut)]
struct EditorText(pub String);

// Local counter to assign unique IDs to preview requests
#[derive(Resource, Default, Deref, DerefMut)]
struct NextRequestId(u64);

#[derive(Resource, Default, Clone, Deref, DerefMut)]
pub struct ErrorMessage(pub String);

#[derive(Resource, Default, Clone, Deref, DerefMut)]
pub struct CurrentFilePath(pub Option<PathBuf>);

#[derive(Resource, Default, Clone, Deref, DerefMut)]
pub struct PendingPreviewStates(pub Vec<PreviewState>);
