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

fn default_entity() -> Entity {
    Entity::PLACEHOLDER
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PreviewBase {
    #[serde(default)]
    pub preview_id: u64,
    pub query: String,
    pub zoom: f32,
    pub rotate_x: f64,
    pub rotate_y: f64,
    #[serde(default)]
    pub order: usize,
    #[serde(skip)]
    pub render_layer: usize,
    #[serde(skip)]
    pub mesh_handle: Handle<Mesh>,
    #[serde(skip)]
    pub rt_image: Handle<Image>,
    #[serde(skip)]
    pub rt_size: UVec2,
    #[serde(skip, default = "default_entity")]
    pub camera_entity: Entity,
    #[serde(skip)]
    pub base_camera_distance: f32,
}

impl PreviewBase {
    pub fn new(preview_id: u64, query: String) -> Self {
        Self {
            preview_id,
            query,
            zoom: 10.0,
            rotate_x: 0.0,
            rotate_y: 0.0,
            order: 0,
            render_layer: 0,
            mesh_handle: Handle::default(),
            rt_image: Handle::default(),
            rt_size: UVec2::ZERO,
            camera_entity: Entity::PLACEHOLDER,
            base_camera_distance: 0.0,
        }
    }
}

#[derive(Component, Serialize, Deserialize, Clone)]
pub enum PreviewTarget {
    Normal {
        base: PreviewBase,
        #[serde(default)]
        control_point_overrides: HashMap<String, f64>,
        #[serde(default)]
        query_param_overrides: HashMap<String, f64>,
        #[serde(skip)]
        evaluated_nodes: Vec<EvaluatedNode>,
        #[serde(skip)]
        control_points: Vec<ControlPoint>,
        #[serde(skip)]
        control_sphere_entities: Vec<Entity>,
        #[serde(skip)]
        query_params: Vec<QueryParam>,
        #[serde(skip)]
        bom_entries: Vec<BomEntry>,
        #[serde(skip)]
        cp_generate_mode: bool,
    },
    Collision {
        base: PreviewBase,
        #[serde(skip)]
        collision_mesh_entities: Vec<Entity>,
        #[serde(skip)]
        collision_count: usize,
        #[serde(skip)]
        part_count: usize,
    },
}

impl PreviewTarget {
    pub fn base(&self) -> &PreviewBase {
        match self {
            PreviewTarget::Normal { base, .. } => base,
            PreviewTarget::Collision { base, .. } => base,
        }
    }
    pub fn base_mut(&mut self) -> &mut PreviewBase {
        match self {
            PreviewTarget::Normal { base, .. } => base,
            PreviewTarget::Collision { base, .. } => base,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SessionPreviews {
    pub previews: Vec<PreviewTarget>,
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
pub struct PendingPreviewStates(pub HashMap<u64, PreviewTarget>);

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
