use std::sync::Arc;
use bevy::log::{info, warn};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

pub fn setup(mut commands: Commands) {
    // 3D camera for rendering model previews
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(6.0, 6.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    // Basic light
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
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
