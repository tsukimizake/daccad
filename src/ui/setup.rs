use crate::ui::ModelPreviews;
use bevy::prelude::*;

pub fn setup(mut commands: Commands) {
    // Main window camera (renders to the primary window)
    commands.spawn((Camera2d::default(),));

    // Start with no previews; they are added via UI
    commands.insert_resource(ModelPreviews::default());
}
