use std::sync::Arc;

use crate::ui::{EditorText, ModelPreviews};
use bevy::log::{info, warn};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

pub fn setup(mut commands: Commands) {
    // Main window camera (renders to the primary window)
    commands.spawn((Camera2d::default(),));

    // Start with no previews; they are added via UI
    commands.insert_resource(ModelPreviews::default());
    // Global editor text for the left pane
    commands.insert_resource(EditorText("cube(10).".to_string()));
}

pub fn setup_fonts(mut contexts: EguiContexts) {
    if let Ok(ctx) = contexts.ctx_mut() {
        let mut fonts = egui::FontDefinitions::default();

        let path = "assets/fonts/NotoSansJP-Regular.ttf";

        if let Ok(bytes) = std::fs::read(path) {
            info!("egui: font loaded: {}", path);
            fonts
                .font_data
                .insert("jp".to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
            if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                list.insert(0, "jp".to_owned());
            }
            if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                list.insert(0, "jp".to_owned());
            }
            ctx.set_fonts(fonts);
            return;
        }
        warn!("egui: no Japanese font found under assets/fonts; tofu may appear");
    } else {
        warn!("egui: no egui context available to set fonts");
    }
}
