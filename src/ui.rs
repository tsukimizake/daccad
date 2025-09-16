use bevy::ecs::component::Component;
use bevy::prelude::*;
use derived_deref::{Deref, DerefMut};

pub mod setup;
pub mod update;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
        use setup::*;
        use update::*;

        app.add_plugins(EguiPlugin::default())
            .add_systems(Startup, setup)
            .add_systems(EguiPrimaryContextPass, setup_fonts.run_if(run_once))
            .add_systems(EguiPrimaryContextPass, egui_ui)
            .add_systems(Update, (on_preview_generated, update_preview_transforms))
            .insert_resource(PreviewTargets::default())
            .insert_resource(EditorText("main :- cube(10).".to_string()))
            .insert_resource(NextRequestId::default());
    }
}

#[derive(Component)]
struct Ui;

#[derive(Resource, Clone, Default, Deref, DerefMut)]
pub(super) struct PreviewTargets(pub Vec<PreviewTarget>);

#[derive(Clone)]
pub(super) struct PreviewTarget {
    pub mesh_handle: Handle<Mesh>,
    pub rt_image: Handle<Image>,
    pub rt_size: UVec2,
    pub rotate_x: f64,
    pub rotate_y: f64,
    pub query: String, // prolog query string to generate the preview. currently unused.
}

#[derive(Resource, Default, Clone, Deref, DerefMut)]
pub(super) struct EditorText(pub String);

// Local counter to assign unique IDs to preview requests
#[derive(Resource, Default)]
pub(super) struct NextRequestId(u64);
