use bevy::ecs::component::Component;
use bevy::prelude::*;

pub mod setup;
pub mod update;

#[derive(Component)]
pub struct Ui;

#[derive(Resource, Default, Clone)]
pub struct ModelPreviews(pub Vec<ModelPreview>);

#[derive(Resource, Default, Clone)]
pub struct EditorText(pub String);

#[derive(Resource, Default, Clone)]
pub struct FontsConfigured(pub bool);

#[derive(Clone)]
pub struct ModelPreview {
    pub image: Handle<Image>,
    pub size: UVec2,
}
