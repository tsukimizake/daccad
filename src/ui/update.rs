use crate::events::{CadhrLangOutput, GeneratePreviewRequest, PreviewGenerated};
use crate::ui::{
    CurrentFilePath, EditorText, ErrorMessage, FreeRenderLayers, NextPreviewId,
    PendingPreviewStates, PreviewState, PreviewTarget, SessionLoadContents, SessionPreviews,
    SessionSaveContents, ThreeMfFileContents,
};
use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::camera::primitives::MeshAabb;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy_egui::{EguiContexts, egui};
use bevy_file_dialog::prelude::*;
use std::io::Cursor;

const MAX_CAMERA_PITCH: f64 = std::f64::consts::FRAC_PI_2 - 0.001;
const MIN_CAMERA_PITCH: f64 = -MAX_CAMERA_PITCH;
const MAX_CAMERA_YAW: f64 = std::f64::consts::PI - 0.001;
const MIN_CAMERA_YAW: f64 = -MAX_CAMERA_YAW;

// egui UI: add previews dynamically and render all existing previews
pub(super) fn egui_ui(
    mut commands: Commands,
    mut contexts: EguiContexts,
    mut preview_query: Query<(Entity, &mut PreviewTarget)>,
    mut editor_text: ResMut<EditorText>,
    mut next_preview_id: ResMut<NextPreviewId>,
    mut pending_states: ResMut<PendingPreviewStates>,
    mut free_render_layers: ResMut<FreeRenderLayers>,
    mut ev_generate: MessageWriter<GeneratePreviewRequest>,
    error_message: Res<ErrorMessage>,
    current_file_path: Res<CurrentFilePath>,
    meshes: Res<Assets<Mesh>>,
) {
    // Collect preview targets into a Vec for indexed access
    let mut preview_targets: Vec<(Entity, Mut<PreviewTarget>)> = preview_query.iter_mut().collect();
    // Toolbar: add a new preview or reload existing
    if let Ok(ctx) = contexts.ctx_mut() {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Session operations
                if ui.button("Open Session").clicked() {
                    commands
                        .dialog()
                        .pick_directory_path::<SessionLoadContents>();
                }
                if ui.button("Save Session").clicked() {
                    if let Some(ref path) = **current_file_path {
                        save_session(
                            path,
                            &editor_text,
                            preview_targets.iter().map(|(_, t)| t.as_ref()),
                        );
                    } else {
                        commands
                            .dialog()
                            .set_file_name("untitled")
                            .save_file::<SessionSaveContents>(vec![]);
                    }
                }
                if ui.button("Save Session As").clicked() {
                    let file_name = current_file_path
                        .as_ref()
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("untitled");
                    commands
                        .dialog()
                        .set_file_name(file_name)
                        .save_file::<SessionSaveContents>(vec![]);
                }

                ui.separator();

                if ui.button("Add Preview").clicked() {
                    let preview_id = **next_preview_id;
                    **next_preview_id += 1;
                    let query_text = "main.".to_string();
                    pending_states.insert(
                        preview_id,
                        PreviewState {
                            preview_id: Some(preview_id),
                            query: query_text.clone(),
                            zoom: 10.0,
                            rotate_x: 0.0,
                            rotate_y: 0.0,
                        },
                    );
                    ev_generate.write(GeneratePreviewRequest {
                        preview_id,
                        database: (**editor_text).clone(),
                        query: query_text,
                    });
                }
                if ui.button("Update Previes").clicked() {
                    // Re-render all previews with the current editor text
                    for (_, target) in preview_targets.iter() {
                        ev_generate.write(GeneratePreviewRequest {
                            preview_id: target.preview_id,
                            database: (**editor_text).clone(),
                            query: target.query.clone(),
                        });
                    }
                }
            });
        });
    }

    // Precompute egui texture ids for each preview's offscreen image
    let preview_images: Vec<(egui::TextureId, UVec2)> = preview_targets
        .iter()
        .map(|(_, t)| {
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
                // Collect actions to process after iterating
                let mut updates_to_send: Vec<(u64, String)> = Vec::new();
                let mut exports_to_send: Vec<usize> = Vec::new();
                let mut closes_to_send: Vec<(Entity, usize)> = Vec::new();
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(right, |ui| {
                        if preview_targets.is_empty() {
                            ui.label(
                                "プレビューはまだありません。上の『Add Preview』を押してください。",
                            );
                        } else {
                            for (i, (entity, target)) in preview_targets.iter_mut().enumerate() {
                                if let Some((tex_id, size)) = preview_images.get(i) {
                                    match preview_target_ui(ui, i, target, *tex_id, *size) {
                                        PreviewAction::Update => {
                                            updates_to_send
                                                .push((target.preview_id, target.query.clone()));
                                        }
                                        PreviewAction::Export3MF => {
                                            exports_to_send.push(i);
                                        }
                                        PreviewAction::Close => {
                                            closes_to_send.push((*entity, target.render_layer));
                                        }
                                        PreviewAction::None => {}
                                    }
                                }
                                ui.add_space(6.0);
                            }
                        }
                    });
                // Send update requests
                for (preview_id, query) in updates_to_send {
                    ev_generate.write(GeneratePreviewRequest {
                        preview_id,
                        database: (**editor_text).clone(),
                        query,
                    });
                }
                // Handle export requests
                for idx in exports_to_send {
                    if let Some(target) = preview_targets.get(idx) {
                        if let Some(mesh) = meshes.get(&target.1.mesh_handle) {
                            if let Some(threemf_data) = bevy_mesh_to_threemf(mesh) {
                                let file_name = current_file_path
                                    .as_ref()
                                    .as_ref()
                                    .and_then(|p| p.file_stem())
                                    .and_then(|n| n.to_str())
                                    .map(|s| format!("{}_preview{}.3mf", s, idx + 1))
                                    .unwrap_or_else(|| format!("preview{}.3mf", idx + 1));
                                commands
                                    .dialog()
                                    .add_filter("3MF", &["3mf"])
                                    .set_file_name(file_name)
                                    .save_file::<ThreeMfFileContents>(threemf_data);
                            }
                        }
                    }
                }
                for (entity, render_layer) in closes_to_send {
                    free_render_layers.push(render_layer);
                    commands.entity(entity).despawn();
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
    mut preview_query: Query<&mut PreviewTarget>,
    mut pending_states: ResMut<PendingPreviewStates>,
    mut free_render_layers: ResMut<FreeRenderLayers>,
) {
    for ev in ev_generated.read() {
        // Update existing preview if it is still alive.
        let mut updated_existing = false;
        for target in preview_query.iter_mut() {
            if target.preview_id == ev.preview_id {
                if let Some(mesh_asset) = meshes.get_mut(&target.mesh_handle) {
                    *mesh_asset = ev.mesh.clone();
                }
                updated_existing = true;
                break;
            }
        }
        if updated_existing {
            continue;
        }

        // Spawn only when this ID is still pending creation.
        let Some(pending_state) = pending_states.remove(&ev.preview_id) else {
            continue;
        };

        // New preview: spawn entities
        let mesh_handle = meshes.add(ev.mesh.clone());

        // Choose a simple material
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.2, 0.2),
            ..default()
        });

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
        let Some(render_layer) = free_render_layers.pop() else {
            bevy::log::error!(
                "No free render layer available for preview {}",
                ev.preview_id
            );
            continue;
        };
        let layer_only = RenderLayers::layer(render_layer);

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
            camera_distance,
            camera_distance * 0.5,
        );

        // Make the mesh visible to both default (0) and offscreen layer
        let both_layers = RenderLayers::from_layers(&[0, render_layer]);

        // Spawn XYZ axis indicators
        let axis_length = 20.0;
        let axis_radius = 0.1;
        let axis_cylinder = meshes.add(Cylinder::new(axis_radius, axis_length));

        let x_material = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.0, 0.0),
            unlit: true,
            ..default()
        });
        let y_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.0, 1.0, 0.0),
            unlit: true,
            ..default()
        });
        let z_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.0, 0.0, 1.0),
            unlit: true,
            ..default()
        });

        // Spawn root entity with all children
        let mut camera_entity = Entity::PLACEHOLDER;
        commands
            .spawn((Transform::default(), Visibility::default()))
            .with_children(|parent| {
                // Mesh
                parent.spawn((
                    Mesh3d(mesh_handle.clone()),
                    MeshMaterial3d(material),
                    Transform::from_xyz(0.0, 0.0, 0.0),
                    both_layers.clone(),
                ));

                // Camera
                camera_entity = parent
                    .spawn((
                        Camera3d::default(),
                        Camera::default(),
                        RenderTarget::Image(rt_image.clone().into()),
                        Transform::from_xyz(cam_pos.x, cam_pos.y, cam_pos.z)
                            .looking_at(Vec3::ZERO, Vec3::Z),
                        layer_only.clone(),
                    ))
                    .id();

                // Light
                parent.spawn((
                    DirectionalLight::default(),
                    Transform::from_xyz(4.0, 4.0, 8.0).looking_at(Vec3::ZERO, Vec3::Z),
                    layer_only.clone(),
                ));

                // X axis (red)
                parent.spawn((
                    Mesh3d(axis_cylinder.clone()),
                    MeshMaterial3d(x_material),
                    Transform::from_xyz(axis_length / 2.0, 0.0, 0.0)
                        .with_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)),
                    both_layers.clone(),
                ));

                // Y axis (green)
                parent.spawn((
                    Mesh3d(axis_cylinder.clone()),
                    MeshMaterial3d(y_material),
                    Transform::from_xyz(0.0, axis_length / 2.0, 0.0),
                    both_layers.clone(),
                ));

                // Z axis (blue)
                parent.spawn((
                    Mesh3d(axis_cylinder.clone()),
                    MeshMaterial3d(z_material),
                    Transform::from_xyz(0.0, 0.0, axis_length / 2.0)
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    both_layers.clone(),
                ));
            })
            .insert({
                PreviewTarget {
                    preview_id: ev.preview_id,
                    render_layer,
                    mesh_handle: mesh_handle.clone(),
                    rt_image: rt_image.clone(),
                    rt_size,
                    camera_entity,
                    base_camera_distance: camera_distance,
                    zoom: pending_state.zoom,
                    rotate_x: pending_state.rotate_x,
                    rotate_y: pending_state.rotate_y,
                    query: ev.query.clone(),
                }
            });
    }
}

// Pending previews and polling system are no longer needed with bevy-async-ecs

/// UI action returned from preview_target_ui
enum PreviewAction {
    None,
    Update,
    Export3MF,
    Close,
}

/// Returns the action requested by the user
fn preview_target_ui(
    ui: &mut egui::Ui,
    index: usize,
    target: &mut PreviewTarget,
    tex_id: egui::TextureId,
    size: UVec2,
) -> PreviewAction {
    let mut action = PreviewAction::None;
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
                if ui.button("Update Preview").clicked() {
                    action = PreviewAction::Update;
                }
                if ui.button("Export 3MF").clicked() {
                    action = PreviewAction::Export3MF;
                }
                if ui.button("Close").clicked() {
                    action = PreviewAction::Close;
                }
            });
            ui.add_space(4.0);
            // Camera controls (orbit and zoom)
            ui.horizontal(|ui| {
                target.rotate_x = target.rotate_x.clamp(MIN_CAMERA_PITCH, MAX_CAMERA_PITCH);
                target.rotate_y = target.rotate_y.clamp(MIN_CAMERA_YAW, MAX_CAMERA_YAW);
                ui.label("Rotate X:");
                ui.add(
                    egui::DragValue::new(&mut target.rotate_x)
                        .speed(0.01)
                        .range(MIN_CAMERA_PITCH..=MAX_CAMERA_PITCH),
                );
                ui.label("Rotate Y:");
                ui.add(
                    egui::DragValue::new(&mut target.rotate_y)
                        .speed(0.01)
                        .range(MIN_CAMERA_YAW..=MAX_CAMERA_YAW),
                );
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
    action
}

// Keep spawned preview entity rotations in sync with UI values
pub(super) fn update_preview_transforms(
    preview_query: Query<&PreviewTarget>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
) {
    for target in preview_query.iter() {
        if let Ok(mut transform) = camera_query.get_mut(target.camera_entity) {
            let rx = target.rotate_x.clamp(MIN_CAMERA_PITCH, MAX_CAMERA_PITCH) as f32;
            let ry = target.rotate_y.clamp(MIN_CAMERA_YAW, MAX_CAMERA_YAW) as f32;
            let dist = target.base_camera_distance * (20.0 / target.zoom);

            // Orbit camera around origin
            let x = dist * ry.sin() * rx.cos();
            let y = dist * ry.cos() * rx.cos();
            let z = dist * rx.sin();

            transform.translation = Vec3::new(x, y, z);
            *transform = transform.looking_at(Vec3::ZERO, Vec3::Z);
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

fn save_session<'a>(
    dir: &std::path::Path,
    editor_text: &EditorText,
    preview_targets: impl Iterator<Item = &'a PreviewTarget>,
) {
    // Remove the marker file if it exists (from save_file dialog)
    let _ = std::fs::remove_file(dir);

    if let Err(e) = std::fs::create_dir_all(dir) {
        bevy::log::error!("Failed to create session directory: {:?}", e);
        return;
    }

    let db_path = dir.join("db.cadhr");
    let previews_path = dir.join("previews.json");

    if let Err(e) = std::fs::write(&db_path, &**editor_text) {
        bevy::log::error!("Failed to save db file: {:?}", e);
        return;
    }

    let previews = SessionPreviews {
        previews: preview_targets
            .map(|t| PreviewState {
                preview_id: Some(t.preview_id),
                query: t.query.clone(),
                zoom: t.zoom,
                rotate_x: t.rotate_x,
                rotate_y: t.rotate_y,
            })
            .collect(),
    };

    match serde_json::to_string_pretty(&previews) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&previews_path, json) {
                bevy::log::error!("Failed to save previews.json: {:?}", e);
            }
        }
        Err(e) => bevy::log::error!("Failed to serialize previews: {:?}", e),
    }
}

fn load_session(dir: &std::path::Path) -> Option<(String, SessionPreviews)> {
    let db_path = dir.join("db.cadhr");
    let previews_path = dir.join("previews.json");

    let db_content = std::fs::read_to_string(&db_path).ok()?;
    let previews_json = std::fs::read_to_string(&previews_path).ok()?;
    let previews: SessionPreviews = serde_json::from_str(&previews_json).ok()?;

    Some((db_content, previews))
}

pub(super) fn session_saved(
    mut ev_saved: MessageReader<DialogFileSaved<SessionSaveContents>>,
    mut current_file_path: ResMut<CurrentFilePath>,
    editor_text: Res<EditorText>,
    preview_query: Query<&PreviewTarget>,
) {
    for ev in ev_saved.read() {
        if ev.result.is_ok() {
            save_session(&ev.path, &editor_text, preview_query.iter());
            **current_file_path = Some(ev.path.clone());
        }
    }
}

pub(super) fn session_loaded(
    mut commands: Commands,
    mut ev_picked: MessageReader<DialogDirectoryPicked<SessionLoadContents>>,
    mut editor_text: ResMut<EditorText>,
    mut current_file_path: ResMut<CurrentFilePath>,
    preview_query: Query<(Entity, &PreviewTarget)>,
    mut ev_generate: MessageWriter<GeneratePreviewRequest>,
    mut next_preview_id: ResMut<NextPreviewId>,
    mut pending_states: ResMut<PendingPreviewStates>,
    mut free_render_layers: ResMut<FreeRenderLayers>,
) {
    for ev in ev_picked.read() {
        if let Some((db_content, previews)) = load_session(&ev.path) {
            **editor_text = db_content;
            **current_file_path = Some(ev.path.clone());

            // Despawn all preview root entities (children are automatically removed)
            for (entity, target) in preview_query.iter() {
                free_render_layers.push(target.render_layer);
                commands.entity(entity).despawn();
            }

            pending_states.clear();

            for mut preview_state in previews.previews {
                let preview_id = if let Some(saved_id) = preview_state.preview_id {
                    if saved_id >= **next_preview_id {
                        **next_preview_id = saved_id.saturating_add(1);
                    }
                    saved_id
                } else {
                    let generated_id = **next_preview_id;
                    **next_preview_id += 1;
                    generated_id
                };

                preview_state.preview_id = Some(preview_id);
                let query = preview_state.query.clone();
                pending_states.insert(preview_id, preview_state);

                ev_generate.write(GeneratePreviewRequest {
                    preview_id,
                    database: (**editor_text).clone(),
                    query,
                });
            }
        } else {
            bevy::log::error!("Failed to load session from {:?}", ev.path);
        }
    }
}

pub(super) fn threemf_saved(mut ev_saved: MessageReader<DialogFileSaved<ThreeMfFileContents>>) {
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
        triangles: threemf::model::Triangles {
            triangle: triangles,
        },
    };

    // Write to buffer
    let mut buffer = Cursor::new(Vec::new());
    if threemf::write(&mut buffer, threemf_mesh).is_err() {
        return None;
    }

    Some(buffer.into_inner())
}
