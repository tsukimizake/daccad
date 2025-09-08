use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::{camera::RenderTarget, render_resource::*, view::RenderLayers};
use bevy_egui::{EguiContexts, egui};

use crate::ui::{EditorText, ModelPreview, ModelPreviews};

// egui UI: add previews dynamically and render all existing previews
pub fn egui_ui(
    mut contexts: EguiContexts,
    mut previews: ResMut<ModelPreviews>,
    mut editor_text: ResMut<EditorText>,
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
                });
            }
        });
    }

    // Precompute texture ids while we don't hold a ctx borrow
    let preview_textures: Vec<(egui::TextureId, UVec2)> = previews
        .0
        .iter()
        .map(|p| {
            let id = contexts
                .image_id(&p.image)
                .unwrap_or_else(|| contexts.add_image(p.image.clone()));
            (id, p.size)
        })
        .collect();

    // Main split view: left = large text area, right = previews
    if let Ok(ctx) = contexts.ctx_mut() {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |columns| {
                // Left half: big multiline text area
                let left = &mut columns[0];
                let size_left = left.available_size();
                left.add_sized(
                    size_left,
                    egui::TextEdit::multiline(&mut editor_text.0)
                        .hint_text("ここにテキストを入力してください"),
                );

                // Right half: show all previews stacked
                let right = &mut columns[1];
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(right, |ui| {
                        if preview_textures.is_empty() {
                            ui.label(
                                "プレビューはまだありません。上の『Add Preview』を押してください。",
                            );
                        } else {
                            for (i, (tex_id, size)) in preview_textures.iter().enumerate() {
                                egui::Frame::default()
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(120)))
                                    .corner_radius(egui::CornerRadius::same(6))
                                    .inner_margin(egui::Margin::symmetric(8, 8))
                                    .show(ui, |ui| {
                                        ui.label(format!("Preview {}", i + 1));
                                        let avail_w = ui.available_width();
                                        let (w, h) = {
                                            let w = avail_w.max(1.0);
                                            let aspect = size.y as f32 / size.x as f32;
                                            (w, w * aspect)
                                        };
                                        ui.add(egui::Image::from_texture((
                                            *tex_id,
                                            egui::vec2(w, h),
                                        )));
                                    });
                                ui.add_space(6.0);
                            }
                        }
                    });
            });
        });
    }
}
