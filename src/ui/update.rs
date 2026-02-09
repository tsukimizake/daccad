use crate::events::{CadhrLangOutput, GeneratePreviewRequest, PreviewGenerated};
use crate::ui::{
    CurrentFilePath, EditorText, ErrorMessage, NextRequestId, PreviewTarget, PreviewTargets,
    CadhrLangFileContents, ThreeMfFileContents,
};
use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::camera::primitives::MeshAabb;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy_egui::{EguiContexts, egui};
use bevy_file_dialog::prelude::*;
use std::io::Cursor;

// egui UI: add previews dynamically and render all existing previews
pub(super) fn egui_ui(
    mut commands: Commands,
    mut contexts: EguiContexts,
    mut preview_targets: ResMut<PreviewTargets>,
    mut editor_text: ResMut<EditorText>,
    mut next_id: ResMut<NextRequestId>,
    mut ev_generate: MessageWriter<GeneratePreviewRequest>,
    error_message: Res<ErrorMessage>,
    current_file_path: Res<CurrentFilePath>,
    meshes: Res<Assets<Mesh>>,
) {
    // Toolbar: add a new preview or reload existing
    if let Ok(ctx) = contexts.ctx_mut() {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // File operations
                if ui.button("Open").clicked() {
                    commands
                        .dialog()
                        .add_filter("CadhrLang", &["cad", "cadhr"])
                        .add_filter("All", &["*"])
                        .load_file::<CadhrLangFileContents>();
                }
                if ui.button("Save").clicked() {
                    if let Some(ref path) = **current_file_path {
                        let _ = std::fs::write(path, &**editor_text);
                    } else {
                        commands
                            .dialog()
                            .add_filter("CadhrLang", &["cad", "cadhr"])
                            .set_file_name("untitled.cadhr")
                            .save_file::<CadhrLangFileContents>(editor_text.as_bytes().to_vec());
                    }
                }
                if ui.button("Save As").clicked() {
                    let file_name = current_file_path
                        .as_ref()
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("untitled.cadhr");
                    commands
                        .dialog()
                        .add_filter("CadhrLang", &["cad", "cadhr"])
                        .set_file_name(file_name)
                        .save_file::<CadhrLangFileContents>(editor_text.as_bytes().to_vec());
                }

                ui.separator();

                if ui.button("Add Preview").clicked() {
                    let id = **next_id;
                    **next_id += 1;
                    let query_text = "main.".to_string();
                    ev_generate.write(GeneratePreviewRequest {
                        request_id: id,
                        database: (**editor_text).clone(),
                        query: query_text,
                        preview_index: None,
                    });
                }
                if ui.button("Reload").clicked() {
                    // Re-render all previews with the current editor text
                    for (i, t) in preview_targets.iter().enumerate() {
                        let id = **next_id;
                        **next_id += 1;
                        ev_generate.write(GeneratePreviewRequest {
                            request_id: id,
                            database: (**editor_text).clone(),
                            query: t.query.clone(),
                            preview_index: Some(i),
                        });
                    }
                }

                ui.separator();

                // Export 3MF button - exports the first preview's mesh
                if ui.button("Export 3MF").clicked() {
                    if let Some(target) = preview_targets.first() {
                        if let Some(mesh) = meshes.get(&target.mesh_handle) {
                            if let Some(threemf_data) = bevy_mesh_to_threemf(mesh) {
                                commands
                                    .dialog()
                                    .add_filter("3MF", &["3mf"])
                                    .set_file_name("export.3mf")
                                    .save_file::<ThreeMfFileContents>(threemf_data);
                            }
                        }
                    }
                }
            });
        });
    }

    // Precompute egui texture ids for each preview's offscreen image
    let preview_images: Vec<(egui::TextureId, UVec2)> = preview_targets
        .iter()
        .map(|t| {
            let id = contexts.image_id(&t.rt_image).unwrap_or_else(|| {
                contexts.add_image(bevy_egui::EguiTextureHandle::Strong(t.rt_image.clone()))
            });
            (id, t.rt_size)
        })
        .collect();

    // Error message panel at the bottom
    if let Ok(ctx) = contexts.ctx_mut() {
        if !error_message.is_empty() {
            egui::TopBottomPanel::bottom("error_panel")
                .min_height(24.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(egui::Color32::from_rgb(255, 100, 100), &**error_message);
                    });
                });
        }
    }

    // Main split view: left = large text area, right = previews list and controls
    if let Ok(ctx) = contexts.ctx_mut() {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |columns| {
                // Left half: big multiline text area
                let left = &mut columns[0];
                let size_left = left.available_size();
                left.add_sized(
                    size_left,
                    egui::TextEdit::multiline(&mut **editor_text)
                        .hint_text("ここにテキストを入力してください"),
                );

                // Right half: show and edit previews
                let right = &mut columns[1];
                // Collect update requests to send after iterating
                let mut updates_to_send: Vec<(usize, String)> = Vec::new();
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(right, |ui| {
                        if preview_targets.is_empty() {
                            ui.label(
                                "プレビューはまだありません。上の『Add Preview』を押してください。",
                            );
                        } else {
                            for (i, t) in preview_targets.iter_mut().enumerate() {
                                if let Some((tex_id, size)) = preview_images.get(i) {
                                    if preview_target_ui(ui, i, t, *tex_id, *size) {
                                        updates_to_send.push((i, t.query.clone()));
                                    }
                                }
                                ui.add_space(6.0);
                            }
                        }
                    });
                // Send update requests
                for (idx, query) in updates_to_send {
                    let id = **next_id;
                    **next_id += 1;
                    ev_generate.write(GeneratePreviewRequest {
                        request_id: id,
                        database: (**editor_text).clone(),
                        query,
                        preview_index: Some(idx),
                    });
                }
            });
        });
    }
}

// Handle generated previews: spawn entities and track UI state
pub(super) fn on_preview_generated(
    mut ev_generated: MessageReader<PreviewGenerated>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut preview_targets: ResMut<PreviewTargets>,
) {
    for ev in ev_generated.read() {
        // Check if this is an update to an existing preview
        if let Some(idx) = ev.preview_index {
            if let Some(target) = preview_targets.get_mut(idx) {
                // Update the existing mesh asset
                if let Some(mesh_asset) = meshes.get_mut(&target.mesh_handle) {
                    *mesh_asset = ev.mesh.clone();
                }
                continue;
            }
        }

        // New preview: spawn entities
        let mesh_handle = meshes.add(ev.mesh.clone());

        // Choose a simple material
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.2, 0.2),
            ..default()
        });

        // Spawn the visible mesh entity at origin
        let entity = commands
            .spawn((
                Mesh3d(mesh_handle.clone()),
                MeshMaterial3d(material),
                Transform::from_xyz(0.0, 0.0, 0.0),
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
        image.texture_descriptor.usage = TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC;
        let rt_image = images.add(image);

        // Unique render layer per preview
        let layer_idx = (preview_targets.len() as u8).saturating_add(1);
        let layer_only = RenderLayers::layer(layer_idx as usize);

        // Calculate camera distance based on mesh bounds
        let mesh_aabb = ev.mesh.compute_aabb();
        let camera_distance = if let Some(aabb) = mesh_aabb {
            let half_extents = aabb.half_extents;
            let max_extent = half_extents.x.max(half_extents.y).max(half_extents.z);
            // Use FOV of ~45 degrees, so distance = extent / tan(22.5°) ≈ extent * 2.4
            // Add some margin (1.5x) for comfortable viewing
            let distance = max_extent * 2.4 * 1.5;
            distance.max(5.0) // Minimum distance
        } else {
            5.0
        };
        let cam_pos = Vec3::new(
            camera_distance * 0.5,
            camera_distance * 0.5,
            camera_distance,
        );

        // Offscreen camera rendering only that layer
        let camera_entity = commands
            .spawn((
                Camera3d::default(),
                Camera::default(),
                RenderTarget::Image(rt_image.clone().into()),
                Transform::from_xyz(cam_pos.x, cam_pos.y, cam_pos.z)
                    .looking_at(Vec3::ZERO, Vec3::Y),
                layer_only.clone(),
            ))
            .id();

        // Light for the offscreen layer
        commands.spawn((
            DirectionalLight::default(),
            Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
            layer_only.clone(),
        ));

        // Make the mesh visible to both default (0) and offscreen layer
        let both_layers = RenderLayers::from_layers(&[0, layer_idx as usize]);
        commands.entity(entity).insert(both_layers.clone());

        // Spawn XYZ axis indicators
        let axis_length = 20.0;
        let axis_radius = 0.1;
        let axis_cylinder = meshes.add(Cylinder::new(axis_radius, axis_length));

        // X axis (red)
        let x_material = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.0, 0.0),
            unlit: true,
            ..default()
        });
        commands.spawn((
            Mesh3d(axis_cylinder.clone()),
            MeshMaterial3d(x_material),
            Transform::from_xyz(axis_length / 2.0, 0.0, 0.0)
                .with_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)),
            both_layers.clone(),
        ));

        // Y axis (green)
        let y_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.0, 1.0, 0.0),
            unlit: true,
            ..default()
        });
        commands.spawn((
            Mesh3d(axis_cylinder.clone()),
            MeshMaterial3d(y_material),
            Transform::from_xyz(0.0, axis_length / 2.0, 0.0),
            both_layers.clone(),
        ));

        // Z axis (blue)
        let z_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.0, 0.0, 1.0),
            unlit: true,
            ..default()
        });
        commands.spawn((
            Mesh3d(axis_cylinder.clone()),
            MeshMaterial3d(z_material),
            Transform::from_xyz(0.0, 0.0, axis_length / 2.0)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            both_layers.clone(),
        ));

        // Store in resource for UI display and transform updates
        preview_targets.push(PreviewTarget {
            mesh_handle: mesh_handle.clone(),
            rt_image: rt_image.clone(),
            rt_size,
            camera_entity,
            base_camera_distance: camera_distance,
            zoom: 10.0,
            rotate_x: 0.0,
            rotate_y: 0.0,
            query: ev.query.clone(),
        });
    }
}

// Pending previews and polling system are no longer needed with bevy-async-ecs

/// Returns true if the Update button was clicked
fn preview_target_ui(
    ui: &mut egui::Ui,
    index: usize,
    target: &mut PreviewTarget,
    tex_id: egui::TextureId,
    size: UVec2,
) -> bool {
    let mut update_clicked = false;
    egui::Frame::default()
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(120)))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(8, 8))
        .show(ui, |ui| {
            ui.label(format!("Preview {}", index + 1));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("?-");
                ui.text_edit_singleline(&mut target.query);
                if ui.button("Update").clicked() {
                    update_clicked = true;
                }
            });
            ui.add_space(4.0);
            // Camera controls (orbit and zoom)
            ui.horizontal(|ui| {
                ui.label("Rotate X:");
                ui.add(egui::DragValue::new(&mut target.rotate_x).speed(0.01));
                ui.label("Rotate Y:");
                ui.add(egui::DragValue::new(&mut target.rotate_y).speed(0.01));
                ui.label("Zoom:");
                ui.add(
                    egui::DragValue::new(&mut target.zoom)
                        .speed(0.1)
                        .range(1.0..=100.0),
                );
            });
            ui.add_space(6.0);
            // Show the offscreen render under controls
            let avail_w = ui.available_width();
            let aspect = size.y as f32 / size.x as f32;
            let w = avail_w;
            let h = w * aspect;
            ui.add(egui::Image::from_texture((tex_id, egui::vec2(w, h))));
        });
    update_clicked
}

// Keep spawned preview entity rotations in sync with UI values
pub(super) fn update_preview_transforms(
    preview_targets: Res<PreviewTargets>,
    mut q: Query<&mut Transform, With<Camera3d>>,
) {
    for target in preview_targets.iter() {
        if let Ok(mut transform) = q.get_mut(target.camera_entity) {
            let rx = target.rotate_x as f32;
            let ry = target.rotate_y as f32;
            let dist = target.base_camera_distance * (20.0 / target.zoom);

            // Orbit camera around origin
            let x = dist * ry.sin() * rx.cos();
            let y = dist * rx.sin();
            let z = dist * ry.cos() * rx.cos();

            transform.translation = Vec3::new(x, y, z);
            *transform = transform.looking_at(Vec3::ZERO, Vec3::Y);
        }
    }
}

// Handle cadhr-lang output messages and update error display
pub(super) fn handle_cadhr_lang_output(
    mut ev_output: MessageReader<CadhrLangOutput>,
    mut error_message: ResMut<ErrorMessage>,
) {
    for output in ev_output.read() {
        if output.is_error {
            **error_message = output.message.clone();
        } else {
            // For non-error logs, we could display them differently or just log
            bevy::log::info!("CadhrLang: {}", output.message);
            // Clear error message on successful execution
            **error_message = String::new();
        }
    }
}

pub(super) fn file_loaded(
    mut ev_loaded: MessageReader<DialogFileLoaded<CadhrLangFileContents>>,
    mut editor_text: ResMut<EditorText>,
    mut current_file_path: ResMut<CurrentFilePath>,
) {
    for ev in ev_loaded.read() {
        if let Ok(content) = std::str::from_utf8(&ev.contents) {
            **editor_text = content.to_string();
            **current_file_path = Some(ev.path.clone());
        }
    }
}

pub(super) fn file_saved(
    mut ev_saved: MessageReader<DialogFileSaved<CadhrLangFileContents>>,
    mut current_file_path: ResMut<CurrentFilePath>,
) {
    for ev in ev_saved.read() {
        if ev.result.is_ok() {
            **current_file_path = Some(ev.path.clone());
        }
    }
}

pub(super) fn threemf_saved(
    mut ev_saved: MessageReader<DialogFileSaved<ThreeMfFileContents>>,
) {
    for ev in ev_saved.read() {
        if ev.result.is_ok() {
            bevy::log::info!("3MF file saved to: {:?}", ev.path);
        } else {
            bevy::log::error!("Failed to save 3MF file: {:?}", ev.result);
        }
    }
}

/// Convert a Bevy Mesh to 3MF file bytes
fn bevy_mesh_to_threemf(mesh: &Mesh) -> Option<Vec<u8>> {
    // Get positions
    let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION)? {
        VertexAttributeValues::Float32x3(pos) => pos,
        _ => return None,
    };

    // Get indices
    let indices = match mesh.indices()? {
        Indices::U32(idx) => idx.clone(),
        Indices::U16(idx) => idx.iter().map(|&i| i as u32).collect(),
    };

    // Convert to threemf types
    let vertices: Vec<threemf::model::Vertex> = positions
        .iter()
        .map(|[x, y, z]| threemf::model::Vertex {
            x: *x as f64,
            y: *y as f64,
            z: *z as f64,
        })
        .collect();

    let triangles: Vec<threemf::model::Triangle> = indices
        .chunks(3)
        .map(|tri| threemf::model::Triangle {
            v1: tri[0] as usize,
            v2: tri[1] as usize,
            v3: tri[2] as usize,
        })
        .collect();

    let threemf_mesh = threemf::model::Mesh {
        vertices: threemf::model::Vertices { vertex: vertices },
        triangles: threemf::model::Triangles { triangle: triangles },
    };

    // Write to buffer
    let mut buffer = Cursor::new(Vec::new());
    if threemf::write(&mut buffer, threemf_mesh).is_err() {
        return None;
    }

    Some(buffer.into_inner())
}