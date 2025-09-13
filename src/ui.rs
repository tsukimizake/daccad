use bevy::ecs::component::Component;
use bevy::prelude::*;

pub mod setup;
pub mod update;

#[derive(Component)]
pub struct Ui;

#[derive(Resource, Default, Clone)]
pub struct PreviewTargets(pub Vec<PreviewTarget>);

#[derive(Clone)]
pub struct PreviewTarget {
    pub mesh_handle: Handle<Mesh>,
    pub rt_image: Handle<Image>,
    pub rt_size: UVec2,
    pub rotate_x: f64,
    pub rotate_y: f64,
    pub query: String, // prolog query string to generate the preview. currently unused.
}

#[derive(Resource, Default, Clone)]
pub struct EditorText(pub String);
