use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::{
    camera::RenderTarget,
    render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    view::RenderLayers,
};
use bevy::tasks::AsyncComputeTaskPool;
use bevy_async_ecs::AsyncWorld;
use bevy_egui::{EguiContexts, egui};

use crate::prolog::mock::mock_generate_mesh;

use crate::ui::{EditorText, PreviewTarget, PreviewTargets};

#[derive(Resource, Clone)]
pub struct AsyncWorldRes(pub AsyncWorld);

// egui UI: add previews dynamically and render all existing previews
pub fn egui_ui(
    mut contexts: EguiContexts,
    mut preview_targets: ResMut<PreviewTargets>,
    mut editor_text: ResMut<EditorText>,
    async_world: Res<AsyncWorldRes>,
) {
    // Toolbar: add a new preview
    if let Ok(ctx) = contexts.ctx_mut() {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            if ui.button("Add Preview").clicked() {
                let query_text = editor_text.0.clone();
                let async_world = async_world.0.clone();
                let fut = async move {
                    let mesh = mock_generate_mesh().await;
                    async_world
                        .apply(|world: &mut World| {
                            finalize_add_preview_world(world, mesh, query_text);
                        })
                        .await;
                };
                AsyncComputeTaskPool::get().spawn(fut).detach();
            }
        });
    }

    // Precompute egui texture ids for each preview's offscreen image
    let preview_images: Vec<(egui::TextureId, UVec2)> = preview_targets
        .0
        .iter()
        .map(|t| {
            let id = contexts
                .image_id(&t.rt_image)
                .unwrap_or_else(|| contexts.add_image(t.rt_image.clone()));
            (id, t.rt_size)
        })
        .collect();

    // Main split view: left = large text area, right = previews list and controls
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

                // Right half: show and edit previews
                let right = &mut columns[1];
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(right, |ui| {
                        if preview_targets.0.is_empty() {
                            ui.label(
                                "プレビューはまだありません。上の『Add Preview』を押してください。",
                            );
                        } else {
                            for (i, t) in preview_targets.0.iter_mut().enumerate() {
                                if let Some((tex_id, size)) = preview_images.get(i) {
                                    preview_target_ui(ui, i, t, *tex_id, *size);
                                }
                                ui.add_space(6.0);
                            }
                        }
                    });
            });
        });
    }
}

fn finalize_add_preview_world(world: &mut World, mesh: Mesh, query_text: String) {
    // Store generated mesh
    let mesh_handle = {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        meshes.add(mesh)
    };
    // Choose a simple material
    let material = {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.2, 0.2),
        ..default()
        })
    };

    // Position new preview based on current count
    let idx = { world.resource::<PreviewTargets>().0.len() };
    let x = (idx as f32) * 2.5 - 2.5;

    // Spawn the visible mesh entity in the 3D world
    let entity = world
        .spawn((
            Mesh3d(mesh_handle.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(x, 0.5, 0.0),
        ))
        .id();

    // Create an offscreen render target
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
    image.texture_descriptor.usage =
        TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC;
    let rt_image = {
        let mut images = world.resource_mut::<Assets<Image>>();
        images.add(image)
    };

    // Unique render layer per preview
    let layer_idx = ({ world.resource::<PreviewTargets>().0.len() } as u8).saturating_add(1);
    let layer_only = RenderLayers::layer(layer_idx as usize);

    // Offscreen camera rendering only that layer
    world.spawn((
        Camera3d::default(),
        Camera {
            target: RenderTarget::Image(rt_image.clone().into()),
            ..default()
        },
        Transform::from_xyz(2.5, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        layer_only.clone(),
    ));

    // Light for the offscreen layer
    world.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        layer_only.clone(),
    ));

    // Make the mesh visible to both default (0) and offscreen layer
    let both_layers = RenderLayers::from_layers(&[0, layer_idx as usize]);
    world.entity_mut(entity).insert(both_layers);

    // Store in resource for UI display and transform updates
    world.resource_mut::<PreviewTargets>().0.push(PreviewTarget {
        mesh_handle: mesh_handle.clone(),
        rt_image: rt_image.clone(),
        rt_size,
        rotate_x: 0.0,
        rotate_y: 0.0,
        // initialize with query captured when task started
        query: query_text,
    });
}

// Pending previews and polling system are no longer needed with bevy-async-ecs

fn preview_target_ui(
    ui: &mut egui::Ui,
    index: usize,
    target: &mut PreviewTarget,
    tex_id: egui::TextureId,
    size: UVec2,
) {
    egui::Frame::default()
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(120)))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(8, 8))
        .show(ui, |ui| {
            ui.label(format!("Preview {}", index + 1));
            ui.add_space(4.0);
            // Query text
            ui.label("Query:");
            ui.text_edit_singleline(&mut target.query);
            ui.add_space(4.0);
            // Rotation controls
            ui.horizontal(|ui| {
                ui.label("Rotate X:");
                ui.add(egui::DragValue::new(&mut target.rotate_x).speed(0.01));
                ui.label("Rotate Y:");
                ui.add(egui::DragValue::new(&mut target.rotate_y).speed(0.01));
            });
            ui.add_space(6.0);
            // Show the offscreen render under controls
            let avail_w = ui.available_width();
            let aspect = size.y as f32 / size.x as f32;
            let w = avail_w.clamp(160.0, 640.0);
            let h = w * aspect;
            ui.add(egui::Image::from_texture((tex_id, egui::vec2(w, h))));
        });
}

// Keep spawned preview entity rotations in sync with UI values
pub fn update_preview_transforms(
    preview_targets: Res<PreviewTargets>,
    mut q: Query<(&Mesh3d, &mut Transform)>,
) {
    if preview_targets.0.is_empty() {
        return;
    }
    for (mesh3d, mut transform) in q.iter_mut() {
        if let Some(t) = preview_targets
            .0
            .iter()
            .find(|t| t.mesh_handle.id() == mesh3d.0.id())
        {
            let rx = t.rotate_x as f32;
            let ry = t.rotate_y as f32;
            transform.rotation = Quat::from_euler(EulerRot::XYZ, rx, ry, 0.0);
        }
    }
}
