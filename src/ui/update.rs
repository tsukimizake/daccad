use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::{camera::RenderTarget, render_resource::*, view::RenderLayers};
use bevy_egui::{EguiContexts, egui};

use crate::ui::{ModelPreview, ModelPreviews};

// egui UI: add previews dynamically and render all existing previews
pub fn egui_ui(
    mut contexts: EguiContexts,
    mut previews: ResMut<ModelPreviews>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if let Ok(ctx) = contexts.ctx_mut() {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            if ui.button("Add Preview").clicked() {
                // Create an offscreen render target for this preview
                let rt_size = UVec2::new(512, 384);
                let size = Extent3d {
                    width: rt_size.x,
                    height: rt_size.y,
                    depth_or_array_layers: 1,
                };
                let mut image = Image::new_fill(
                    size,
                    TextureDimension::D2,
                    &[0, 0, 0, 0],
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
                );
                image.texture_descriptor.usage = TextureUsages::RENDER_ATTACHMENT
                    | TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_SRC;
                let rt_image = images.add(image);

                // Unique render layer for this preview
                let layer_idx = (previews.0.len() as u8).saturating_add(1);
                let layer = RenderLayers::layer(layer_idx as usize);

                // Camera rendering into the texture
                commands.spawn((
                    Camera3d::default(),
                    Camera {
                        target: RenderTarget::Image(rt_image.clone().into()),
                        ..default()
                    },
                    Transform::from_xyz(2.5, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
                    layer.clone(),
                ));

                // Light for this layer
                commands.spawn((
                    DirectionalLight::default(),
                    Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
                    layer.clone(),
                ));

                // Cube in the preview layer
                let mesh = meshes.add(Mesh::from(Cuboid::from_size(Vec3::splat(1.0))));
                let material = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.7, 0.2, 0.2),
                    ..default()
                });
                commands.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_xyz(0.0, 0.5, 0.0),
                    layer.clone(),
                ));

                // Track this preview for UI
                previews.0.push(ModelPreview {
                    image: rt_image,
                    size: rt_size,
                    text: String::new(),
                });
            }
        });
    }

    // Draw all previews (separate borrows of contexts per window)
    for (i, preview) in previews.0.iter_mut().enumerate() {
        let tex_id = contexts
            .image_id(&preview.image)
            .unwrap_or_else(|| contexts.add_image(preview.image.clone()));

        if let Ok(ctx) = contexts.ctx_mut() {
            egui::Window::new(format!("Preview {}", i + 1))
                .default_open(true)
                .show(ctx, |ui| {
                    egui::Frame::default()
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(120)))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin::symmetric(8, 8))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut preview.text)
                                        .hint_text("Enter text"),
                                );
                                let size = [preview.size.x as f32, preview.size.y as f32];
                                ui.add(egui::Image::from_texture((
                                    tex_id,
                                    egui::vec2(size[0], size[1]),
                                )));
                            });
                        });
                });
        }
    }
}
