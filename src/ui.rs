use bevy::prelude::*;
use bevy_file_dialog::prelude::*;
use cadhr_lang::bom::BomEntry;
use cadhr_lang::manifold_bridge::{ControlPoint, EvaluatedNode};
use cadhr_lang::parse::QueryParam;
use derived_deref::{Deref, DerefMut};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

pub mod setup;
pub mod update;

pub struct UiPlugin;

#[derive(Resource, Default)]
pub struct UnsavedChanges {
    pub dirty: bool,
    pub show_close_dialog: bool,
}

pub struct ThreeMfFileContents;
pub struct BomJsonFileContents;

pub struct SessionSaveContents;
pub struct SessionLoadContents;

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub enum PreviewModeType {
    #[default]
    Normal,
    Collision,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PreviewState {
    #[serde(default)]
    pub preview_id: Option<u64>,
    pub query: String,
    pub zoom: f32,
    pub rotate_x: f64,
    pub rotate_y: f64,
    #[serde(default)]
    pub control_point_overrides: HashMap<String, f64>,
    #[serde(default)]
    pub query_param_overrides: HashMap<String, f64>,
    #[serde(default)]
    pub mode: PreviewModeType,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SessionPreviews {
    pub previews: Vec<PreviewState>,
}

pub enum PreviewMode {
    Normal {
        evaluated_nodes: Vec<EvaluatedNode>,
        control_points: Vec<ControlPoint>,
        control_sphere_entities: Vec<Entity>,
        control_point_overrides: HashMap<String, f64>,
        query_params: Vec<QueryParam>,
        query_param_overrides: HashMap<String, f64>,
        bom_entries: Vec<BomEntry>,
        cp_generate_mode: bool,
    },
    Collision {
        collision_mesh_entities: Vec<Entity>,
        collision_count: usize,
        part_count: usize,
    },
}

/// Preview target data stored as a component on the root entity.
/// When this entity is despawned, all children are automatically removed.
#[derive(Component)]
pub struct PreviewTarget {
    pub preview_id: u64,
    pub render_layer: usize,
    pub mesh_handle: Handle<Mesh>,
    pub rt_image: Handle<Image>,
    pub rt_size: UVec2,
    pub camera_entity: Entity,
    pub base_camera_distance: f32,
    pub zoom: f32,
    pub rotate_x: f64,
    pub rotate_y: f64,
    pub query: String,
    pub mode: PreviewMode,
}

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
        use setup::*;
        use update::{
            auto_reload_system, bom_json_saved, egui_ui, handle_cadhr_lang_output,
            handle_close_requested, on_collision_preview_generated, on_preview_generated,
            restore_last_session, session_loaded, session_saved, threemf_saved,
            update_preview_transforms,
        };

        app.add_plugins(EguiPlugin::default())
            .add_plugins(
                FileDialogPlugin::new()
                    .with_save_file::<SessionSaveContents>()
                    .with_pick_directory::<SessionLoadContents>()
                    .with_save_file::<ThreeMfFileContents>()
                    .with_save_file::<BomJsonFileContents>(),
            )
            .add_systems(Startup, (setup, restore_last_session))
            .add_systems(EguiPrimaryContextPass, setup_fonts.run_if(run_once))
            .add_systems(EguiPrimaryContextPass, egui_ui)
            .add_systems(
                Update,
                (
                    on_preview_generated,
                    on_collision_preview_generated,
                    update_preview_transforms,
                    handle_cadhr_lang_output,
                ),
            )
            .add_systems(
                Update,
                (
                    session_saved,
                    session_loaded,
                    threemf_saved,
                    bom_json_saved,
                    auto_reload_system,
                    handle_close_requested,
                ),
            )
            .insert_resource(EditorText("main :- cube(10, 20, 30).".to_string()))
            .insert_resource(NextPreviewId::default())
            .insert_resource(FreeRenderLayers::default())
            .insert_resource(ErrorMessage::default())
            .insert_resource(CurrentFilePath::default())
            .insert_resource(PendingPreviewStates::default())
            .insert_resource(SelectedControlPoint::default())
            .insert_resource(AutoReload::default())
            .insert_resource(UnsavedChanges::default());
    }
}

#[derive(Resource, Default)]
pub struct SelectedControlPoint {
    pub preview_id: Option<u64>,
    pub index: usize,
}

#[derive(Resource, Default, Clone, Deref, DerefMut)]
struct EditorText(pub String);

#[derive(Resource, Default, Deref, DerefMut)]
pub struct NextPreviewId(u64);

#[derive(Resource, Clone, Deref, DerefMut)]
pub struct FreeRenderLayers(pub Vec<usize>);

impl Default for FreeRenderLayers {
    fn default() -> Self {
        Self((1..32).rev().collect())
    }
}

#[derive(Resource, Default, Clone)]
pub struct ErrorMessage {
    pub message: String,
    pub span: Option<cadhr_lang::parse::SrcSpan>,
}

#[derive(Resource, Default, Clone, Deref, DerefMut)]
pub struct CurrentFilePath(pub Option<PathBuf>);

#[derive(Resource, Default, Clone, Deref, DerefMut)]
pub struct PendingPreviewStates(pub HashMap<u64, PreviewState>);

#[derive(Resource)]
pub struct AutoReload {
    pub enabled: bool,
    pub last_modified: Option<SystemTime>,
}

impl Default for AutoReload {
    fn default() -> Self {
        Self {
            enabled: false,
            last_modified: None,
        }
    }
}
