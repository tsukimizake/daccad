use bevy::prelude::*;
use bevy_file_dialog::prelude::*;
use derived_deref::{Deref, DerefMut};
use std::path::PathBuf;

pub mod setup;
pub mod update;

pub struct UiPlugin;

pub struct PrologFileContents;

pub struct ThreeMfFileContents;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
        use setup::*;
        use update::{egui_ui, file_loaded, file_saved, handle_prolog_output, on_preview_generated, update_preview_transforms, threemf_saved};

        app.add_plugins(EguiPlugin::default())
            .add_plugins(
                FileDialogPlugin::new()
                    .with_save_file::<PrologFileContents>()
                    .with_load_file::<PrologFileContents>()
                    .with_save_file::<ThreeMfFileContents>(),
            )
            .add_systems(Startup, setup)
            .add_systems(EguiPrimaryContextPass, setup_fonts.run_if(run_once))
            .add_systems(EguiPrimaryContextPass, egui_ui)
            .add_systems(Update, (on_preview_generated, update_preview_transforms, handle_prolog_output))
            .add_systems(Update, (file_loaded, file_saved, threemf_saved))
            .insert_resource(PreviewTargets::default())
            .insert_resource(EditorText("main :- cube(10, 20, 30).".to_string()))
            .insert_resource(NextRequestId::default())
            .insert_resource(ErrorMessage::default())
            .insert_resource(CurrentFilePath::default());
    }
}

#[derive(Resource, Clone, Default, Deref, DerefMut)]
struct PreviewTargets(pub Vec<PreviewTarget>);

#[derive(Clone)]
struct PreviewTarget {
    pub mesh_handle: Handle<Mesh>,
    pub rt_image: Handle<Image>,
    pub rt_size: UVec2,
    pub camera_entity: Entity,
    pub base_camera_distance: f32, // calculated from mesh size
    pub zoom: f32,                 // 1-100, default 10
    pub rotate_x: f64,
    pub rotate_y: f64,
    pub query: String, // prolog query string to generate the preview.
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