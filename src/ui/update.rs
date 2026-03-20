use crate::events::{
    CadhrLangOutput, CollisionPreviewGenerated, GenerateCollisionPreviewRequest,
    GeneratePreviewRequest, PreviewGenerated,
};

fn snap_to_char_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        s.len()
    } else {
        let mut i = idx;
        while !s.is_char_boundary(i) {
            i += 1;
        }
        i
    }
}
use crate::ui::{
    AutoReload, BomJsonFileContents, CurrentFilePath, EditorText, ErrorMessage, FreeRenderLayers,
    NextPreviewId, PendingPreviewStates, PreviewBase, PreviewClickMode, PreviewTarget,
    SelectedControlPoint, SessionLoadContents, SessionPreviews, SessionSaveContents,
    ThreeMfFileContents, UnsavedChanges,
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
use cadhr_lang::manifold_bridge::EvaluatedNode;
use cadhr_lang::parse::SrcSpan;
use std::io::Cursor;
use std::path::Path;

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct PreviewEvents<'w> {
    generate: MessageWriter<'w, GeneratePreviewRequest>,
    collision: MessageWriter<'w, GenerateCollisionPreviewRequest>,
}

const MAX_CAMERA_PITCH: f64 = std::f64::consts::FRAC_PI_2 - 0.001;
const MIN_CAMERA_PITCH: f64 = -MAX_CAMERA_PITCH;
const MAX_CAMERA_YAW: f64 = std::f64::consts::PI - 0.001;
const MIN_CAMERA_YAW: f64 = -MAX_CAMERA_YAW;
pub(crate) const DEFAULT_ZOOM: f32 = 10.0;
const MIN_ZOOM: f32 = 1.0;
const MAX_ZOOM: f32 = 100.0;
const CONTROL_SPHERE_RADIUS: f32 = 0.5;
const CONTROL_SPHERE_HIT_RADIUS: f64 = 1.5;
const CONTROL_SPHERE_COLOR: Color = Color::srgb(1.0, 0.9, 0.0);
const CONTROL_SPHERE_SELECTED_COLOR: Color = Color::srgb(0.0, 1.0, 0.5);
const CAMERA_DISTANCE_FACTOR: f32 = 2.4 * 3.0;
const MIN_CAMERA_DISTANCE: f32 = 5.0;

fn camera_distance_from_mesh(mesh: &Mesh) -> f32 {
    if let Some(aabb) = mesh.compute_aabb() {
        let half_extents = aabb.half_extents;
        let max_extent = half_extents.x.max(half_extents.y).max(half_extents.z);
        (max_extent * CAMERA_DISTANCE_FACTOR).max(MIN_CAMERA_DISTANCE)
    } else {
        MIN_CAMERA_DISTANCE
    }
}

// egui UI: add previews dynamically and render all existing previews
pub(super) fn egui_ui(
    mut commands: Commands,
    mut contexts: EguiContexts,
    mut preview_query: Query<(Entity, &mut PreviewTarget)>,
    mut editor_text: ResMut<EditorText>,
    mut next_preview_id: ResMut<NextPreviewId>,
    mut pending_states: ResMut<PendingPreviewStates>,
    mut free_render_layers: ResMut<FreeRenderLayers>,
    mut preview_events: PreviewEvents,
    mut selected_cp: ResMut<SelectedControlPoint>,
    mut error_message: ResMut<ErrorMessage>,
    mut current_file_path: ResMut<CurrentFilePath>,
    mut auto_reload: ResMut<AutoReload>,
    mut unsaved: ResMut<UnsavedChanges>,
    mut app_exit: MessageWriter<bevy::app::AppExit>,
    meshes: Res<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Collect preview targets into a Vec for indexed access, sorted by order
    let mut preview_targets: Vec<(Entity, Mut<PreviewTarget>)> = preview_query.iter_mut().collect();
    preview_targets.sort_by_key(|(_, t)| t.base().order);
    // Toolbar: menu bar + preview buttons
    if let Ok(ctx) = contexts.ctx_mut() {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Session").clicked() {
                        ui.close();
                        **editor_text = "main :- cube(10, 20, 30).".to_string();
                        **current_file_path = None;
                        unsaved.dirty = false;
                        **next_preview_id = 0;
                        pending_states.clear();
                        *selected_cp = SelectedControlPoint::default();
                        *error_message = ErrorMessage::default();
                        for (target_id, target) in preview_targets.drain(..) {
                            free_render_layers.push(target.base().render_layer);
                            commands.entity(target_id).despawn();
                        }
                    }
                    if ui.button("Open Session").clicked() {
                        ui.close();
                        commands
                            .dialog()
                            .pick_directory_path::<SessionLoadContents>();
                    }
                    if ui.button("Save Session").clicked() {
                        ui.close();
                        if let Some(ref path) = **current_file_path {
                            save_session(
                                path,
                                &editor_text,
                                preview_targets.iter().map(|(_, t)| t.as_ref()),
                            );
                            unsaved.dirty = false;
                        } else {
                            commands
                                .dialog()
                                .set_file_name("untitled")
                                .save_file::<SessionSaveContents>(vec![]);
                        }
                    }
                    if ui.button("Save Session As").clicked() {
                        ui.close();
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
                    let mut enabled = auto_reload.enabled;
                    if ui.checkbox(&mut enabled, "Auto Reload").clicked() {
                        auto_reload.enabled = enabled;
                        if enabled {
                            // 有効化時に現在のファイルの更新時刻を記録
                            auto_reload.last_modified =
                                current_file_path.as_ref().as_ref().and_then(|p| {
                                    std::fs::metadata(p.join("db.cadhr"))
                                        .and_then(|m| m.modified())
                                        .ok()
                                });
                        }
                    }
                });

                ui.separator();

                if ui.button("Add Preview").clicked() {
                    let preview_id = **next_preview_id;
                    **next_preview_id += 1;
                    let query_text = "main.".to_string();
                    pending_states.insert(
                        preview_id,
                        PreviewTarget::Normal {
                            base: PreviewBase::new(preview_id, query_text.clone()),
                            control_point_overrides: Default::default(),
                            query_param_overrides: Default::default(),
                            evaluated_nodes: vec![],
                            control_points: vec![],
                            control_sphere_entities: vec![],
                            query_params: vec![],
                            bom_entries: vec![],
                            click_mode: PreviewClickMode::Normal,
                        },
                    );
                    preview_events.generate.write(GeneratePreviewRequest {
                        preview_id,
                        database: (**editor_text).clone(),
                        query: query_text,
                        include_paths: (**current_file_path).iter().cloned().collect(),
                        control_point_overrides: Default::default(),
                        query_param_overrides: Default::default(),
                    });
                }
                if ui.button("Add Collision Check").clicked() {
                    let preview_id = **next_preview_id;
                    **next_preview_id += 1;
                    let query_text = "main.".to_string();
                    pending_states.insert(
                        preview_id,
                        PreviewTarget::Collision {
                            base: PreviewBase::new(preview_id, query_text.clone()),
                            collision_mesh_entities: vec![],
                            collision_count: 0,
                            part_count: 0,
                        },
                    );
                    preview_events
                        .collision
                        .write(GenerateCollisionPreviewRequest {
                            preview_id,
                            database: (**editor_text).clone(),
                            query: query_text,
                            include_paths: (**current_file_path).iter().cloned().collect(),
                        });
                }
                if ui.button("Update Previews").clicked() {
                    for (_, target) in preview_targets.iter() {
                        send_update_for_target(
                            target,
                            &editor_text,
                            &current_file_path,
                            &mut preview_events.generate,
                            &mut preview_events.collision,
                        );
                    }
                }
            });
        });
    }

    // Precompute egui texture ids for each preview's offscreen image
    let preview_images: Vec<(egui::TextureId, UVec2)> = preview_targets
        .iter()
        .map(|(_, t)| {
            let id = contexts.image_id(&t.base().rt_image).unwrap_or_else(|| {
                contexts.add_image(bevy_egui::EguiTextureHandle::Strong(
                    t.base().rt_image.clone(),
                ))
            });
            (id, t.base().rt_size)
        })
        .collect();

    // Error message panel at the bottom
    if let Ok(ctx) = contexts.ctx_mut() {
        if !error_message.message.is_empty() {
            egui::TopBottomPanel::bottom("error_panel")
                .min_height(24.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 100, 100),
                            &error_message.message,
                        );
                    });
                });
        }
    }

    // Main split view: left = large text area, right = previews list and controls
    if let Ok(ctx) = contexts.ctx_mut() {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |columns| {
                // Left half: text area + editable vars panel
                let left = &mut columns[0];

                // Text editor
                let size_left = left.available_size();
                let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
                    let text = buf.as_str();
                    let mut job = egui::text::LayoutJob::default();
                    job.wrap.max_width = wrap_width;

                    let default_format = egui::TextFormat {
                        font_id: egui::TextFormat::default().font_id,
                        color: ui.visuals().text_color(),
                        ..Default::default()
                    };
                    let error_bg = egui::Color32::from_rgba_unmultiplied(255, 80, 80, 100);
                    let ok_bg = egui::Color32::from_rgba_unmultiplied(0, 80, 0, 100);

                    if let Some(span) = error_message.span {
                        let error_format = egui::TextFormat {
                            background: error_bg,
                            ..default_format.clone()
                        };
                        let start = snap_to_char_boundary(text, span.start.min(text.len()));
                        let end = snap_to_char_boundary(text, span.end.min(text.len()));
                        if 0 < start {
                            job.append(&text[..start], 0.0, default_format.clone());
                        }
                        job.append(&text[start..end], 0.0, error_format);
                        if end < text.len() {
                            job.append(&text[end..], 0.0, default_format);
                        }
                    } else if error_message.message.is_empty() {
                        let ok_format = egui::TextFormat {
                            background: ok_bg,
                            ..default_format
                        };
                        job.append(text, 0.0, ok_format);
                    } else {
                        job.append(text, 0.0, default_format);
                    }
                    ui.fonts_mut(|f| f.layout_job(job))
                };
                let editor_response = left.add_sized(
                    size_left,
                    egui::TextEdit::multiline(&mut **editor_text)
                        .hint_text("ここにテキストを入力してください")
                        .layouter(&mut layouter),
                );
                if editor_response.changed() {
                    unsaved.dirty = true;
                }

                // Right half: show and edit previews
                let right = &mut columns[1];
                // Collect actions to process after iterating
                let mut updates_to_send: Vec<(u64, String)> = Vec::new();
                let mut exports_to_send: Vec<usize> = Vec::new();
                let mut bom_exports_to_send: Vec<usize> = Vec::new();
                let mut closes_to_send: Vec<(Entity, usize)> = Vec::new();
                let mut cp_override_regenerate: Vec<u64> = Vec::new();
                let mut qp_override_regenerate: Vec<u64> = Vec::new();
                let mut cp_source_edits: Vec<(f64, SrcSpan)> = Vec::new();
                let mut dnd_swap: Option<(usize, usize)> = None;
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(right, |ui| {
                        if preview_targets.is_empty() {
                            ui.label(
                                "プレビューはまだありません。上の『Add Preview』を押してください。",
                            );
                        } else {
                            for (i, (target_id, target)) in preview_targets.iter_mut().enumerate() {
                                if let Some((tex_id, size)) = preview_images.get(i) {
                                    let drop_response =
                                        ui.dnd_drop_zone::<usize, ()>(egui::Frame::NONE, |ui| {
                                            match preview_target_ui(
                                                ui,
                                                i,
                                                target,
                                                *tex_id,
                                                *size,
                                                &mut selected_cp,
                                                &mut cp_override_regenerate,
                                                &mut qp_override_regenerate,
                                                &mut cp_source_edits,
                                            ) {
                                                PreviewAction::Update => {
                                                    updates_to_send.push((
                                                        target.base().preview_id,
                                                        target.base().query.clone(),
                                                    ));
                                                }
                                                PreviewAction::Export3MF => {
                                                    exports_to_send.push(i);
                                                }
                                                PreviewAction::ExportBOM => {
                                                    bom_exports_to_send.push(i);
                                                }
                                                PreviewAction::Close => {
                                                    closes_to_send.push((
                                                        *target_id,
                                                        target.base().render_layer,
                                                    ));
                                                }
                                                PreviewAction::InsertControlPoint(x, y, z) => {
                                                    let cp_text = format!(
                                                        ", control({:.2}, {:.2}, {:.2})",
                                                        x, y, z
                                                    );
                                                    insert_control_point_text(
                                                        &mut **editor_text,
                                                        &target.base().query,
                                                        &cp_text,
                                                    );
                                                    updates_to_send.push((
                                                        target.base().preview_id,
                                                        target.base().query.clone(),
                                                    ));
                                                }
                                                PreviewAction::None => {}
                                            }
                                        });
                                    if let Some(dragged_idx) = drop_response.1 {
                                        dnd_swap = Some((*dragged_idx, i));
                                    }
                                }
                                ui.add_space(6.0);
                            }
                        }
                    });
                if let Some((from, to)) = dnd_swap {
                    if from != to {
                        let from_order = preview_targets[from].1.base().order;
                        let to_order = preview_targets[to].1.base().order;
                        preview_targets[from].1.base_mut().order = to_order;
                        preview_targets[to].1.base_mut().order = from_order;
                    }
                }
                // Update control sphere colors based on selection
                for (_, target) in preview_targets.iter() {
                    if let PreviewTarget::Normal {
                        control_sphere_entities,
                        ..
                    } = &**target
                    {
                        let is_selected_preview =
                            selected_cp.preview_id == Some(target.base().preview_id);
                        for (ci, entity) in control_sphere_entities.iter().enumerate() {
                            let selected = is_selected_preview && selected_cp.index == ci;
                            let color = if selected {
                                CONTROL_SPHERE_SELECTED_COLOR
                            } else {
                                CONTROL_SPHERE_COLOR
                            };
                            let mat = materials.add(StandardMaterial {
                                base_color: color,
                                unlit: true,
                                ..default()
                            });
                            commands.entity(*entity).insert(MeshMaterial3d(mat));
                        }
                    }
                }

                // Send update requests
                for (preview_id, query) in updates_to_send {
                    if let Some((_, target)) = preview_targets
                        .iter()
                        .find(|(_, t)| t.base().preview_id == preview_id)
                    {
                        match &**target {
                            PreviewTarget::Normal {
                                control_point_overrides,
                                query_param_overrides,
                                ..
                            } => {
                                preview_events.generate.write(GeneratePreviewRequest {
                                    preview_id,
                                    database: (**editor_text).clone(),
                                    query,
                                    include_paths: (**current_file_path).iter().cloned().collect(),
                                    control_point_overrides: control_point_overrides.clone(),
                                    query_param_overrides: query_param_overrides.clone(),
                                });
                            }
                            PreviewTarget::Collision { .. } => {
                                preview_events
                                    .collision
                                    .write(GenerateCollisionPreviewRequest {
                                        preview_id,
                                        database: (**editor_text).clone(),
                                        query,
                                        include_paths: (**current_file_path)
                                            .iter()
                                            .cloned()
                                            .collect(),
                                    });
                            }
                        }
                    }
                }
                // Regenerate previews that had control point overrides changed
                for preview_id in cp_override_regenerate {
                    if let Some((_, target)) = preview_targets
                        .iter()
                        .find(|(_, t)| t.base().preview_id == preview_id)
                    {
                        if let PreviewTarget::Normal {
                            control_point_overrides,
                            query_param_overrides,
                            ..
                        } = &**target
                        {
                            preview_events.generate.write(GeneratePreviewRequest {
                                preview_id,
                                database: (**editor_text).clone(),
                                query: target.base().query.clone(),
                                include_paths: (**current_file_path).iter().cloned().collect(),
                                control_point_overrides: control_point_overrides.clone(),
                                query_param_overrides: query_param_overrides.clone(),
                            });
                        }
                    }
                }
                // Regenerate previews that had query param overrides changed
                for preview_id in qp_override_regenerate {
                    if let Some((_, target)) = preview_targets
                        .iter()
                        .find(|(_, t)| t.base().preview_id == preview_id)
                    {
                        if let PreviewTarget::Normal {
                            control_point_overrides,
                            query_param_overrides,
                            ..
                        } = &**target
                        {
                            preview_events.generate.write(GeneratePreviewRequest {
                                preview_id,
                                database: (**editor_text).clone(),
                                query: target.base().query.clone(),
                                include_paths: (**current_file_path).iter().cloned().collect(),
                                control_point_overrides: control_point_overrides.clone(),
                                query_param_overrides: query_param_overrides.clone(),
                            });
                        }
                    }
                }
                // Handle export requests
                for idx in exports_to_send {
                    let Some((_, target)) = preview_targets.get(idx) else {
                        continue;
                    };
                    let Some(threemf_data) = meshes
                        .get(&target.base().mesh_handle)
                        .and_then(|mesh| bevy_mesh_to_threemf(mesh))
                    else {
                        continue;
                    };
                    let file_name = export_3mf_file_name(
                        current_file_path.as_ref().as_ref().map(|p| p.as_path()),
                        &target.base().query,
                    );
                    commands
                        .dialog()
                        .add_filter("3MF", &["3mf"])
                        .set_file_name(file_name)
                        .save_file::<ThreeMfFileContents>(threemf_data);
                }
                for idx in bom_exports_to_send {
                    if let Some((_, target)) = preview_targets.get(idx) {
                        if let PreviewTarget::Normal { bom_entries, .. } = &**target {
                            let json = cadhr_lang::bom::bom_entries_to_json(bom_entries);
                            let filename = target.base().query.clone() + "_bom";
                            let file_name = export_3mf_file_name(
                                current_file_path.as_ref().as_ref().map(|p| p.as_path()),
                                &filename,
                            )
                            .replace(".3mf", ".json");
                            commands
                                .dialog()
                                .add_filter("JSON", &["json"])
                                .set_file_name(file_name)
                                .save_file::<BomJsonFileContents>(json.into_bytes());
                        }
                    }
                }
                for (entity, render_layer) in closes_to_send {
                    free_render_layers.push(render_layer);
                    commands.entity(entity).despawn();
                }

                // Apply control point source edits
                if !cp_source_edits.is_empty() {
                    cp_source_edits.sort_by(|a, b| b.1.start.cmp(&a.1.start));
                    for (new_val, span) in &cp_source_edits {
                        let src = &mut **editor_text;
                        if span.end <= src.len() {
                            if span.start == span.end {
                                let insert_str = format!("={}", new_val);
                                src.insert_str(span.start, &insert_str);
                            } else {
                                let new_val_str = format!("{}", new_val);
                                src.replace_range(span.start..span.end, &new_val_str);
                            }
                        }
                    }
                    for (_, target) in preview_targets.iter() {
                        send_update_for_target(
                            target,
                            &editor_text,
                            &current_file_path,
                            &mut preview_events.generate,
                            &mut preview_events.collision,
                        );
                    }
                }
            });
        });
    }

    if unsaved.show_close_dialog {
        if let Ok(ctx) = contexts.ctx_mut() {
            egui::Window::new("Unsaved Changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("There are unsaved changes. Do you want to save before quitting?");
                    ui.horizontal(|ui| {
                        if ui.button("Save & Quit").clicked() {
                            if let Some(ref path) = **current_file_path {
                                save_session(
                                    path,
                                    &editor_text,
                                    preview_targets.iter().map(|(_, t)| t.as_ref()),
                                );
                            }
                            unsaved.dirty = false;
                            unsaved.show_close_dialog = false;
                            app_exit.write(bevy::app::AppExit::Success);
                        }
                        if ui.button("Quit without Saving").clicked() {
                            unsaved.dirty = false;
                            unsaved.show_close_dialog = false;
                            app_exit.write(bevy::app::AppExit::Success);
                        }
                        if ui.button("Cancel").clicked() {
                            unsaved.show_close_dialog = false;
                        }
                    });
                });
        }
    }
}

pub(super) fn handle_close_requested(
    mut close_events: MessageReader<bevy::window::WindowCloseRequested>,
    mut unsaved: ResMut<UnsavedChanges>,
    mut app_exit: MessageWriter<bevy::app::AppExit>,
) {
    for _ev in close_events.read() {
        if unsaved.dirty {
            unsaved.show_close_dialog = true;
        } else {
            app_exit.write(bevy::app::AppExit::Success);
        }
    }
}

fn send_update_for_target(
    target: &PreviewTarget,
    editor_text: &EditorText,
    current_file_path: &CurrentFilePath,
    ev_generate: &mut MessageWriter<GeneratePreviewRequest>,
    ev_collision: &mut MessageWriter<GenerateCollisionPreviewRequest>,
) {
    match target {
        PreviewTarget::Normal {
            base,
            control_point_overrides,
            query_param_overrides,
            ..
        } => {
            ev_generate.write(GeneratePreviewRequest {
                preview_id: base.preview_id,
                database: (**editor_text).clone(),
                query: base.query.clone(),
                include_paths: (**current_file_path).iter().cloned().collect(),
                control_point_overrides: control_point_overrides.clone(),
                query_param_overrides: query_param_overrides.clone(),
            });
        }
        PreviewTarget::Collision { base, .. } => {
            ev_collision.write(GenerateCollisionPreviewRequest {
                preview_id: base.preview_id,
                database: (**editor_text).clone(),
                query: base.query.clone(),
                include_paths: (**current_file_path).iter().cloned().collect(),
            });
        }
    }
}

fn update_control_spheres(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    parent_entity: Entity,
    control_points: &[cadhr_lang::manifold_bridge::ControlPoint],
    control_sphere_entities: &mut Vec<Entity>,
    both_layers: &RenderLayers,
) {
    let cp_count = control_points.len();
    let existing_count = control_sphere_entities.len();

    for (i, cp) in control_points.iter().enumerate() {
        if i < existing_count {
            commands
                .entity(control_sphere_entities[i])
                .insert(Transform::from_xyz(
                    cp.x.value as f32,
                    cp.y.value as f32,
                    cp.z.value as f32,
                ));
        }
    }

    let sphere_mesh = meshes.add(Sphere::new(CONTROL_SPHERE_RADIUS));
    let sphere_material = materials.add(StandardMaterial {
        base_color: CONTROL_SPHERE_COLOR,
        unlit: true,
        ..default()
    });

    for i in existing_count..cp_count {
        let cp = &control_points[i];
        let child_id = commands
            .spawn((
                Mesh3d(sphere_mesh.clone()),
                MeshMaterial3d(sphere_material.clone()),
                Transform::from_xyz(cp.x.value as f32, cp.y.value as f32, cp.z.value as f32),
                both_layers.clone(),
            ))
            .id();
        commands.entity(parent_entity).add_child(child_id);
        control_sphere_entities.push(child_id);
    }

    for i in cp_count..existing_count {
        commands.entity(control_sphere_entities[i]).despawn();
    }
    control_sphere_entities.truncate(cp_count);
}

// Handle generated previews: spawn entities and track UI state
pub(super) fn on_preview_generated(
    mut ev_generated: MessageReader<PreviewGenerated>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut preview_query: Query<(Entity, &mut PreviewTarget)>,
    mut pending_states: ResMut<PendingPreviewStates>,
    mut free_render_layers: ResMut<FreeRenderLayers>,
) {
    for ev in ev_generated.read() {
        // Update existing preview if it is still alive.
        let mut updated_existing = false;
        for (entity, mut target) in preview_query.iter_mut() {
            if target.base().preview_id == ev.preview_id {
                if let Some(mesh_asset) = meshes.get_mut(&target.base().mesh_handle) {
                    *mesh_asset = ev.mesh.clone();
                }
                target.base_mut().base_camera_distance = camera_distance_from_mesh(&ev.mesh);

                let render_layer = target.base().render_layer;
                if let PreviewTarget::Normal {
                    evaluated_nodes,
                    control_points,
                    bom_entries,
                    query_params,
                    control_sphere_entities,
                    ..
                } = &mut *target
                {
                    *evaluated_nodes = ev.evaluated_nodes.clone();
                    *control_points = ev.control_points.clone();
                    *bom_entries = ev.bom_entries.clone();
                    *query_params = ev.query_params.clone();

                    let both_layers = RenderLayers::from_layers(&[0, render_layer]);
                    update_control_spheres(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        entity,
                        control_points,
                        control_sphere_entities,
                        &both_layers,
                    );
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

        let pending_base = pending_state.base();
        let order = pending_base.order;
        let initial_zoom_saved = pending_base.zoom;
        let saved_rotate_x = pending_base.rotate_x;
        let saved_rotate_y = pending_base.rotate_y;
        let (saved_cp_overrides, saved_qp_overrides) = match &pending_state {
            PreviewTarget::Normal {
                control_point_overrides,
                query_param_overrides,
                ..
            } => (
                control_point_overrides.clone(),
                query_param_overrides.clone(),
            ),
            _ => (Default::default(), Default::default()),
        };

        // New preview: spawn entities
        let mesh_handle = meshes.add(ev.mesh.clone());

        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.2, 0.2),
            cull_mode: None,
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

        let camera_distance = camera_distance_from_mesh(&ev.mesh);
        let initial_zoom = if initial_zoom_saved > 0.0 {
            initial_zoom_saved
        } else {
            DEFAULT_ZOOM
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

        // Control point sphere material
        let cp_sphere_mesh = meshes.add(Sphere::new(CONTROL_SPHERE_RADIUS));
        let cp_material = materials.add(StandardMaterial {
            base_color: CONTROL_SPHERE_COLOR,
            unlit: true,
            ..default()
        });

        // Spawn root entity with all children
        let mut camera_entity = Entity::PLACEHOLDER;
        let mut control_sphere_entities: Vec<Entity> = Vec::new();
        commands
            .spawn((Transform::default(), Visibility::default()))
            .with_children(|parent| {
                // Mesh
                parent.spawn((
                    Mesh3d(mesh_handle.clone()),
                    MeshMaterial3d(material.clone()),
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

                // Control point spheres
                for cp in &ev.control_points {
                    let id = parent
                        .spawn((
                            Mesh3d(cp_sphere_mesh.clone()),
                            MeshMaterial3d(cp_material.clone()),
                            Transform::from_xyz(
                                cp.x.value as f32,
                                cp.y.value as f32,
                                cp.z.value as f32,
                            ),
                            both_layers.clone(),
                        ))
                        .id();
                    control_sphere_entities.push(id);
                }
            })
            .insert(PreviewTarget::Normal {
                base: PreviewBase {
                    preview_id: ev.preview_id,
                    render_layer,
                    mesh_handle: mesh_handle.clone(),
                    rt_image: rt_image.clone(),
                    rt_size,
                    camera_entity,
                    base_camera_distance: camera_distance,
                    zoom: initial_zoom,
                    rotate_x: saved_rotate_x,
                    rotate_y: saved_rotate_y,
                    query: ev.query.clone(),
                    order,
                },
                evaluated_nodes: ev.evaluated_nodes.clone(),
                control_points: ev.control_points.clone(),
                control_sphere_entities,
                control_point_overrides: saved_cp_overrides,
                query_params: ev.query_params.clone(),
                query_param_overrides: saved_qp_overrides,
                bom_entries: ev.bom_entries.clone(),
                click_mode: PreviewClickMode::Normal,
            });
    }
}

pub(super) fn on_collision_preview_generated(
    mut ev_generated: MessageReader<CollisionPreviewGenerated>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut preview_query: Query<(Entity, &mut PreviewTarget)>,
    mut pending_states: ResMut<PendingPreviewStates>,
    mut free_render_layers: ResMut<FreeRenderLayers>,
) {
    for ev in ev_generated.read() {
        // Update existing collision preview
        let mut updated_existing = false;
        for (entity, mut target) in preview_query.iter_mut() {
            if target.base().preview_id == ev.preview_id {
                if let Some(mesh_asset) = meshes.get_mut(&target.base().mesh_handle) {
                    *mesh_asset = ev.combined_mesh.clone();
                }
                target.base_mut().base_camera_distance =
                    camera_distance_from_mesh(&ev.combined_mesh);
                let render_layer = target.base().render_layer;

                if let PreviewTarget::Collision {
                    collision_mesh_entities,
                    collision_count,
                    part_count,
                    ..
                } = &mut *target
                {
                    for e in collision_mesh_entities.drain(..) {
                        commands.entity(e).despawn();
                    }

                    let both_layers = RenderLayers::from_layers(&[0, render_layer]);
                    let collision_material = materials.add(StandardMaterial {
                        base_color: Color::srgba(1.0, 0.0, 0.0, 0.3),
                        alpha_mode: AlphaMode::Blend,
                        ..default()
                    });
                    for collision_mesh in &ev.collision_meshes {
                        let mesh_handle = meshes.add(collision_mesh.clone());
                        let child_id = commands
                            .spawn((
                                Mesh3d(mesh_handle),
                                MeshMaterial3d(collision_material.clone()),
                                Transform::default(),
                                both_layers.clone(),
                            ))
                            .id();
                        commands.entity(entity).add_child(child_id);
                        collision_mesh_entities.push(child_id);
                    }

                    *collision_count = ev.collision_meshes.len();
                    *part_count = ev.part_count;
                }

                updated_existing = true;
                break;
            }
        }
        if updated_existing {
            continue;
        }

        // Spawn new collision preview
        let Some(pending_state) = pending_states.remove(&ev.preview_id) else {
            continue;
        };

        let pending_base = pending_state.base();
        let order = pending_base.order;
        let initial_zoom_saved = pending_base.zoom;
        let saved_rotate_x = pending_base.rotate_x;
        let saved_rotate_y = pending_base.rotate_y;

        let mesh_handle = meshes.add(ev.combined_mesh.clone());
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.2, 0.2),
            cull_mode: None,
            ..default()
        });

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

        let Some(render_layer) = free_render_layers.pop() else {
            bevy::log::error!(
                "No free render layer available for collision preview {}",
                ev.preview_id
            );
            continue;
        };
        let layer_only = RenderLayers::layer(render_layer);
        let both_layers = RenderLayers::from_layers(&[0, render_layer]);

        let camera_distance = camera_distance_from_mesh(&ev.combined_mesh);
        let initial_zoom = if initial_zoom_saved > 0.0 {
            initial_zoom_saved
        } else {
            DEFAULT_ZOOM
        };
        let cam_pos = Vec3::new(
            camera_distance * 0.5,
            camera_distance,
            camera_distance * 0.5,
        );

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

        let collision_material = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.0, 0.0, 0.3),
            alpha_mode: AlphaMode::Blend,
            ..default()
        });

        let mut camera_entity = Entity::PLACEHOLDER;
        let mut collision_mesh_entities: Vec<Entity> = Vec::new();
        commands
            .spawn((Transform::default(), Visibility::default()))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(mesh_handle.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                    both_layers.clone(),
                ));

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

                parent.spawn((
                    DirectionalLight::default(),
                    Transform::from_xyz(4.0, 4.0, 8.0).looking_at(Vec3::ZERO, Vec3::Z),
                    layer_only.clone(),
                ));

                // Axes
                parent.spawn((
                    Mesh3d(axis_cylinder.clone()),
                    MeshMaterial3d(x_material),
                    Transform::from_xyz(axis_length / 2.0, 0.0, 0.0)
                        .with_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)),
                    both_layers.clone(),
                ));
                parent.spawn((
                    Mesh3d(axis_cylinder.clone()),
                    MeshMaterial3d(y_material),
                    Transform::from_xyz(0.0, axis_length / 2.0, 0.0),
                    both_layers.clone(),
                ));
                parent.spawn((
                    Mesh3d(axis_cylinder.clone()),
                    MeshMaterial3d(z_material),
                    Transform::from_xyz(0.0, 0.0, axis_length / 2.0)
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    both_layers.clone(),
                ));

                // Collision mesh overlays
                for collision_mesh in &ev.collision_meshes {
                    let cmesh_handle = meshes.add(collision_mesh.clone());
                    let id = parent
                        .spawn((
                            Mesh3d(cmesh_handle),
                            MeshMaterial3d(collision_material.clone()),
                            Transform::default(),
                            both_layers.clone(),
                        ))
                        .id();
                    collision_mesh_entities.push(id);
                }
            })
            .insert(PreviewTarget::Collision {
                base: PreviewBase {
                    preview_id: ev.preview_id,
                    render_layer,
                    mesh_handle: mesh_handle.clone(),
                    rt_image: rt_image.clone(),
                    rt_size,
                    camera_entity,
                    base_camera_distance: camera_distance,
                    zoom: initial_zoom,
                    rotate_x: saved_rotate_x,
                    rotate_y: saved_rotate_y,
                    query: ev.query.clone(),
                    order,
                },
                collision_mesh_entities,
                collision_count: ev.collision_meshes.len(),
                part_count: ev.part_count,
            });
    }
}

// Pending previews and polling system are no longer needed with bevy-async-ecs

/// UI action returned from preview_target_ui
enum PreviewAction {
    None,
    Update,
    Export3MF,
    ExportBOM,
    Close,
    InsertControlPoint(f64, f64, f64),
}

// ============================================================
// Raycast: click-to-source infrastructure
// ============================================================

/// Ray-triangle intersection using Moller-Trumbore algorithm.
/// Returns distance t if hit (t > 0).
fn ray_triangle_intersect(
    ray_origin: &[f64; 3],
    ray_dir: &[f64; 3],
    v0: &[f64; 3],
    v1: &[f64; 3],
    v2: &[f64; 3],
) -> Option<f64> {
    let edge1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
    let edge2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
    let h = cross(ray_dir, &edge2);
    let a = dot(&edge1, &h);
    if a.abs() < 1e-10 {
        return None;
    }
    let f = 1.0 / a;
    let s = [
        ray_origin[0] - v0[0],
        ray_origin[1] - v0[1],
        ray_origin[2] - v0[2],
    ];
    let u = f * dot(&s, &h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(&s, &edge1);
    let v = f * dot(ray_dir, &q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = f * dot(&edge2, &q);
    if t > 1e-10 { Some(t) } else { None }
}

fn cross(a: &[f64; 3], b: &[f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Test ray against AABB, returns true if ray intersects the box.
fn ray_aabb_intersect(
    ray_origin: &[f64; 3],
    ray_dir: &[f64; 3],
    aabb_min: &[f64; 3],
    aabb_max: &[f64; 3],
) -> bool {
    let mut tmin = f64::NEG_INFINITY;
    let mut tmax = f64::INFINITY;
    for i in 0..3 {
        if ray_dir[i].abs() < 1e-12 {
            if ray_origin[i] < aabb_min[i] || ray_origin[i] > aabb_max[i] {
                return false;
            }
        } else {
            let inv_d = 1.0 / ray_dir[i];
            let mut t1 = (aabb_min[i] - ray_origin[i]) * inv_d;
            let mut t2 = (aabb_max[i] - ray_origin[i]) * inv_d;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            tmin = tmin.max(t1);
            tmax = tmax.min(t2);
            if tmin > tmax {
                return false;
            }
        }
    }
    tmax >= 0.0
}

/// Raycast against an EvaluatedNode tree. Returns (distance, node_ref) of closest hit.
fn raycast_evaluated_nodes<'a>(
    ray_origin: &[f64; 3],
    ray_dir: &[f64; 3],
    nodes: &'a [EvaluatedNode],
) -> Option<(f64, &'a EvaluatedNode)> {
    let mut best: Option<(f64, &'a EvaluatedNode)> = None;
    for node in nodes {
        if let Some((t, n)) = raycast_node(ray_origin, ray_dir, node) {
            if best.is_none() || t < best.unwrap().0 {
                best = Some((t, n));
            }
        }
    }
    best
}

fn raycast_node<'a>(
    ray_origin: &[f64; 3],
    ray_dir: &[f64; 3],
    node: &'a EvaluatedNode,
) -> Option<(f64, &'a EvaluatedNode)> {
    if !ray_aabb_intersect(ray_origin, ray_dir, &node.aabb_min, &node.aabb_max) {
        return None;
    }

    // First try children (more specific nodes)
    let mut best: Option<(f64, &'a EvaluatedNode)> = None;
    for child in &node.children {
        if let Some((t, n)) = raycast_node(ray_origin, ray_dir, child) {
            if best.is_none() || t < best.unwrap().0 {
                best = Some((t, n));
            }
        }
    }
    if best.is_some() {
        return best;
    }

    // No child hit: test this node's mesh triangles
    let num_props = 6usize; // xyz + normals after calculate_normals
    let verts = &node.mesh_verts;
    let indices = &node.mesh_indices;
    let mut closest_t = f64::INFINITY;
    for tri in indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let i0 = tri[0] as usize * num_props;
        let i1 = tri[1] as usize * num_props;
        let i2 = tri[2] as usize * num_props;
        if i0 + 2 >= verts.len() || i1 + 2 >= verts.len() || i2 + 2 >= verts.len() {
            continue;
        }
        let v0 = [verts[i0] as f64, verts[i0 + 1] as f64, verts[i0 + 2] as f64];
        let v1 = [verts[i1] as f64, verts[i1 + 1] as f64, verts[i1 + 2] as f64];
        let v2 = [verts[i2] as f64, verts[i2 + 1] as f64, verts[i2 + 2] as f64];
        if let Some(t) = ray_triangle_intersect(ray_origin, ray_dir, &v0, &v1, &v2) {
            if t < closest_t {
                closest_t = t;
            }
        }
    }
    if closest_t < f64::INFINITY {
        Some((closest_t, node))
    } else {
        None
    }
}

/// クエリのfunctor名に対応するclauseの末尾(`.`の直前)にテキストを挿入する。
/// クエリが "main." なら、ソース中の最後の "main :- ..." clause の `.` 直前に挿入。
fn insert_control_point_text(source: &mut String, query: &str, text: &str) {
    let functor = query.trim().trim_end_matches('.');
    let functor = functor.split('(').next().unwrap_or(functor).trim();
    if functor.is_empty() {
        return;
    }

    // ソースを後ろからスキャンし、"functor :- ..." の最後のclauseを探す
    // clause区切りは '.' (文字列リテラル外)
    let mut best_dot_pos: Option<usize> = None;
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // clause開始位置を探す: 空白をスキップしてfunctorで始まるか確認
        while i < bytes.len()
            && (bytes[i] == b' ' || bytes[i] == b'\n' || bytes[i] == b'\r' || bytes[i] == b'\t')
        {
            i += 1;
        }
        let content_start = i;

        let mut in_string = false;
        let mut dot_pos = None;
        while i < bytes.len() {
            if in_string {
                if bytes[i] == b'\\' {
                    i += 1; // エスケープの次の文字をスキップ
                } else if bytes[i] == b'"' {
                    in_string = false;
                }
            } else if bytes[i] == b'"' {
                in_string = true;
            } else if bytes[i] == b'.' {
                let next_is_digit = i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit();
                if !next_is_digit {
                    dot_pos = Some(i);
                    i += 1;
                    break;
                }
            }
            i += 1;
        }

        if let Some(dp) = dot_pos {
            // このclauseのhead functorがマッチするか確認
            let clause_text = &source[content_start..dp];
            let head = clause_text.split(":-").next().unwrap_or(clause_text).trim();
            let head_functor = head.split('(').next().unwrap_or(head).trim();
            if head_functor == functor {
                best_dot_pos = Some(dp);
            }
        } else {
            break;
        }
    }

    if let Some(pos) = best_dot_pos {
        source.insert_str(pos, text);
    }
}

/// Generate a ray from UV coordinates (0..1) in the preview image, given camera params.
fn generate_ray_from_uv(u: f32, v: f32, target: &PreviewTarget) -> ([f64; 3], [f64; 3]) {
    let rx = target
        .base()
        .rotate_x
        .clamp(MIN_CAMERA_PITCH, MAX_CAMERA_PITCH) as f32;
    let ry = target.base().rotate_y.clamp(MIN_CAMERA_YAW, MAX_CAMERA_YAW) as f32;
    let zoom = target.base().zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    let dist = target.base().base_camera_distance * (20.0 / zoom);

    let cam_x = dist * ry.sin() * rx.cos();
    let cam_y = dist * ry.cos() * rx.cos();
    let cam_z = dist * rx.sin();

    let cam_pos = Vec3::new(cam_x, cam_y, cam_z);
    let cam_transform = Transform::from_translation(cam_pos).looking_at(Vec3::ZERO, Vec3::Z);

    // Default perspective FOV (Bevy Camera3d default is 45 degrees vertical)
    let fov_y: f32 = std::f32::consts::FRAC_PI_4;
    let aspect = target.base().rt_size.x as f32 / target.base().rt_size.y as f32;
    let half_h = (fov_y * 0.5).tan();
    let half_w = half_h * aspect;

    // Convert UV (0..1, 0..1) to NDC (-1..1, -1..1), with V flipped (egui top-left origin)
    let ndc_x = u * 2.0 - 1.0;
    let ndc_y = 1.0 - v * 2.0;

    // Ray direction in camera local space
    let local_dir = Vec3::new(ndc_x * half_w, ndc_y * half_h, -1.0).normalize();

    // Transform to world space
    let world_dir = cam_transform.rotation * local_dir;

    let origin = [cam_pos.x as f64, cam_pos.y as f64, cam_pos.z as f64];
    let dir = [world_dir.x as f64, world_dir.y as f64, world_dir.z as f64];
    (origin, dir)
}

/// Returns the action requested by the user
fn preview_target_ui(
    ui: &mut egui::Ui,
    index: usize,
    target: &mut PreviewTarget,
    tex_id: egui::TextureId,
    size: UVec2,
    selected_cp: &mut SelectedControlPoint,
    cp_override_regenerate: &mut Vec<u64>,
    qp_override_regenerate: &mut Vec<u64>,
    cp_source_edits: &mut Vec<(f64, SrcSpan)>,
) -> PreviewAction {
    let mut action = PreviewAction::None;
    egui::Frame::default()
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(120)))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(8, 8))
        .show(ui, |ui| {
            let mode_label = match &*target {
                PreviewTarget::Normal { .. } => format!("Preview {}", index + 1),
                PreviewTarget::Collision {
                    collision_count,
                    part_count,
                    ..
                } => {
                    if *collision_count > 0 {
                        format!(
                            "Collision {} ({} parts, {} collisions)",
                            index + 1,
                            part_count,
                            collision_count
                        )
                    } else {
                        format!("Collision {} ({} parts, OK)", index + 1, part_count)
                    }
                }
            };
            ui.horizontal(|ui| {
                let drag_id = egui::Id::new("preview_dnd").with(target.base().preview_id);
                ui.dnd_drag_source(drag_id, index, |ui| {
                    ui.label("☰");
                });
                ui.label(mode_label);
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("?-");
                ui.text_edit_singleline(&mut target.base_mut().query);
                if ui.button("Update").clicked() {
                    action = PreviewAction::Update;
                }
                ui.menu_button("menu", |ui| {
                    if let PreviewTarget::Normal {
                        base,
                        bom_entries,
                        control_point_overrides,
                        click_mode,
                        ..
                    } = &mut *target
                    {
                        if ui.button("Export 3MF").clicked() {
                            action = PreviewAction::Export3MF;
                            ui.close();
                        }
                        if !bom_entries.is_empty() && ui.button("Export BOM").clicked() {
                            action = PreviewAction::ExportBOM;
                            ui.close();
                        }
                        if !control_point_overrides.is_empty() && ui.button("Reset CPs").clicked() {
                            control_point_overrides.clear();
                            cp_override_regenerate.push(base.preview_id);
                            ui.close();
                        }
                        let cp_label = if *click_mode == PreviewClickMode::CpGenerate {
                            "CP生成 ✓"
                        } else {
                            "CP生成"
                        };
                        if ui.button(cp_label).clicked() {
                            *click_mode = if *click_mode == PreviewClickMode::CpGenerate {
                                PreviewClickMode::Normal
                            } else {
                                PreviewClickMode::CpGenerate
                            };
                            ui.close();
                        }
                        let snap_label = if *click_mode == PreviewClickMode::SnapTranslate {
                            "スナップ ✓"
                        } else {
                            "スナップ"
                        };
                        if ui.button(snap_label).clicked() {
                            *click_mode = if *click_mode == PreviewClickMode::SnapTranslate {
                                PreviewClickMode::Normal
                            } else {
                                PreviewClickMode::SnapTranslate
                            };
                            ui.close();
                        }
                    }
                });
                if ui.button("Close").clicked() {
                    action = PreviewAction::Close;
                    ui.close();
                }
            });
            // Query parameters sliders
            if let PreviewTarget::Normal {
                base,
                query_params,
                query_param_overrides,
                ..
            } = &mut *target
            {
                if !query_params.is_empty() {
                    egui::Frame::default()
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(80)))
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(egui::Margin::symmetric(6, 4))
                        .show(ui, |ui| {
                            ui.label("Parameters");
                            for param in query_params.iter() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{}:", param.name));
                                    let min_f = param
                                        .min
                                        .map(|b| {
                                            if b.inclusive {
                                                b.value.to_f64()
                                            } else {
                                                b.value.to_f64() + 0.01
                                            }
                                        })
                                        .unwrap_or(-10000.0);
                                    let max_f = param
                                        .max
                                        .map(|b| {
                                            if b.inclusive {
                                                b.value.to_f64()
                                            } else {
                                                b.value.to_f64() - 0.01
                                            }
                                        })
                                        .unwrap_or(10000.0);
                                    let default =
                                        param.default_value.map(|dv| dv.to_f64()).unwrap_or_else(
                                            || match (param.min.as_ref(), param.max.as_ref()) {
                                                (Some(min), Some(max)) => {
                                                    (min.value.to_f64() + max.value.to_f64()) / 2.0
                                                }
                                                _ => 0.0,
                                            },
                                        );
                                    let mut val = query_param_overrides
                                        .get(&param.name)
                                        .copied()
                                        .unwrap_or(default);
                                    let changed = ui
                                        .add(
                                            egui::DragValue::new(&mut val)
                                                .speed(0.1)
                                                .range(min_f..=max_f),
                                        )
                                        .changed();
                                    if changed {
                                        query_param_overrides.insert(param.name.clone(), val);
                                        qp_override_regenerate.push(base.preview_id);
                                    }
                                });
                            }
                        });
                    ui.add_space(4.0);
                }
            }
            // Camera controls (orbit and zoom)
            ui.horizontal(|ui| {
                target.base_mut().rotate_x = target
                    .base()
                    .rotate_x
                    .clamp(MIN_CAMERA_PITCH, MAX_CAMERA_PITCH);
                target.base_mut().rotate_y =
                    target.base().rotate_y.clamp(MIN_CAMERA_YAW, MAX_CAMERA_YAW);
                ui.label("Rotate X:");
                ui.add(
                    egui::DragValue::new(&mut target.base_mut().rotate_x)
                        .speed(0.01)
                        .range(MIN_CAMERA_PITCH..=MAX_CAMERA_PITCH),
                );
                ui.label("Rotate Y:");
                ui.add(
                    egui::DragValue::new(&mut target.base_mut().rotate_y)
                        .speed(0.01)
                        .range(MIN_CAMERA_YAW..=MAX_CAMERA_YAW),
                );
                ui.label("Zoom:");
                ui.add(
                    egui::DragValue::new(&mut target.base_mut().zoom)
                        .speed(0.1)
                        .range(MIN_ZOOM..=MAX_ZOOM),
                );
            });
            ui.add_space(6.0);

            // Show the offscreen render under controls (with click detection)
            let avail_w = ui.available_width();
            let aspect = size.y as f32 / size.x as f32;
            let w = avail_w;
            let h = w * aspect;
            let image_response = ui.add(
                egui::Image::from_texture((tex_id, egui::vec2(w, h))).sense(egui::Sense::click()),
            );

            // Click handling: compute ray before borrowing mode
            let click_ray = if image_response.clicked() {
                image_response.interact_pointer_pos().map(|pos| {
                    let rect = image_response.rect;
                    let u = (pos.x - rect.min.x) / rect.width();
                    let v = (pos.y - rect.min.y) / rect.height();
                    generate_ray_from_uv(u, v, target)
                })
            } else {
                None
            };

            if let PreviewTarget::Normal {
                base,
                click_mode,
                evaluated_nodes,
                control_points,
                control_point_overrides,
                ..
            } = &mut *target
            {
                if let Some((ray_origin, ray_dir)) = click_ray {
                    let surface_hit =
                        raycast_evaluated_nodes(&ray_origin, &ray_dir, evaluated_nodes).map(
                            |(t, _node)| {
                                [
                                    ray_origin[0] + t * ray_dir[0],
                                    ray_origin[1] + t * ray_dir[1],
                                    ray_origin[2] + t * ray_dir[2],
                                ]
                            },
                        );
                    match *click_mode {
                        PreviewClickMode::CpGenerate => {
                            if let Some(hit) = surface_hit {
                                action = PreviewAction::InsertControlPoint(hit[0], hit[1], hit[2]);
                            }
                        }
                        PreviewClickMode::SnapTranslate => {
                            if let Some(hit) = surface_hit {
                                if let Some(first) = selected_cp.snap_translate.first_point.take() {
                                    selected_cp.snap_translate.result = Some([
                                        first[0] - hit[0],
                                        first[1] - hit[1],
                                        first[2] - hit[2],
                                    ]);
                                } else {
                                    selected_cp.snap_translate.first_point = Some(hit);
                                }
                            }
                        }
                        PreviewClickMode::Normal if !control_points.is_empty() => {
                            let sphere_radius = CONTROL_SPHERE_HIT_RADIUS;
                            let mut best_hit: Option<(f64, usize)> = None;
                            for (ci, cp) in control_points.iter().enumerate() {
                                let center = [cp.x.value, cp.y.value, cp.z.value];
                                let oc = [
                                    ray_origin[0] - center[0],
                                    ray_origin[1] - center[1],
                                    ray_origin[2] - center[2],
                                ];
                                let a = dot(&ray_dir, &ray_dir);
                                let b = 2.0 * dot(&oc, &ray_dir);
                                let c = dot(&oc, &oc) - sphere_radius * sphere_radius;
                                let discriminant = b * b - 4.0 * a * c;
                                if discriminant >= 0.0 {
                                    let t = (-b - discriminant.sqrt()) / (2.0 * a);
                                    let t = if t > 0.0 {
                                        t
                                    } else {
                                        (-b + discriminant.sqrt()) / (2.0 * a)
                                    };
                                    if t > 0.0 {
                                        if best_hit.is_none() || t < best_hit.unwrap().0 {
                                            best_hit = Some((t, ci));
                                        }
                                    }
                                }
                            }

                            if let Some((_t, ci)) = best_hit {
                                selected_cp.preview_id = Some(base.preview_id);
                                selected_cp.index = ci;
                            } else if selected_cp.preview_id == Some(base.preview_id) {
                                selected_cp.preview_id = None;
                            }
                        }
                        _ => {}
                    }
                }

                if *click_mode == PreviewClickMode::SnapTranslate {
                    ui.add_space(4.0);
                    if selected_cp.snap_translate.first_point.is_some() {
                        ui.label("1点目選択済み — 2点目をクリック");
                    }
                    if let Some(v) = selected_cp.snap_translate.result {
                        let text = format!("translate({:.2}, {:.2}, {:.2})", v[0], v[1], v[2]);
                        ui.horizontal(|ui| {
                            ui.label(&text);
                            if ui.button("Copy").clicked() {
                                ui.ctx().copy_text(text.clone());
                            }
                        });
                    }
                }

                if selected_cp.preview_id == Some(base.preview_id) {
                    if let Some(cp) = control_points.get_mut(selected_cp.index) {
                        ui.add_space(4.0);
                        let label = cp
                            .name
                            .as_deref()
                            .map(|n| format!("Control: {}", n))
                            .unwrap_or_else(|| format!("Control {}", selected_cp.index));
                        ui.label(label);
                        ui.horizontal(|ui| {
                            for (axis_idx, (axis_label, tracked)) in
                                [("X:", &mut cp.x), ("Y:", &mut cp.y), ("Z:", &mut cp.z)]
                                    .iter_mut()
                                    .enumerate()
                            {
                                ui.label(*axis_label);
                                let mut val = cp.var_names[axis_idx]
                                    .as_ref()
                                    .and_then(|vn| control_point_overrides.get(vn).copied())
                                    .unwrap_or(tracked.value);
                                let response = ui.add(egui::DragValue::new(&mut val).speed(0.5));
                                if response.changed() {
                                    if let Some(ref vname) = cp.var_names[axis_idx] {
                                        control_point_overrides.insert(vname.clone(), val);
                                    }
                                    cp_override_regenerate.push(base.preview_id);
                                }
                                if response.drag_stopped() || response.lost_focus() {
                                    if let Some(span) = tracked.source_span {
                                        cp_source_edits.push((val, span));
                                        if let Some(ref vname) = cp.var_names[axis_idx] {
                                            control_point_overrides.remove(vname);
                                        }
                                    }
                                }
                            }
                        });
                    }
                }
            }
        });
    action
}

fn export_3mf_file_name(current_path: Option<&Path>, query: &str) -> String {
    let base_name = current_path
        .and_then(|p| p.file_stem())
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty());
    let query_suffix = sanitize_query_for_filename(query);

    if let Some(base) = base_name {
        format!("{base}_{query_suffix}.3mf")
    } else {
        format!("{query_suffix}.3mf")
    }
}

fn sanitize_query_for_filename(query: &str) -> String {
    query
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

// Keep spawned preview entity rotations in sync with UI values
pub(super) fn update_preview_transforms(
    preview_query: Query<&PreviewTarget>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
) {
    for target in preview_query.iter() {
        if let Ok(mut transform) = camera_query.get_mut(target.base().camera_entity) {
            let rx = target
                .base()
                .rotate_x
                .clamp(MIN_CAMERA_PITCH, MAX_CAMERA_PITCH) as f32;
            let ry = target.base().rotate_y.clamp(MIN_CAMERA_YAW, MAX_CAMERA_YAW) as f32;
            let zoom = target.base().zoom.clamp(MIN_ZOOM, MAX_ZOOM);
            let dist = target.base().base_camera_distance * (20.0 / zoom);

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
    mut preview_query: Query<&mut PreviewTarget>,
) {
    for output in ev_output.read() {
        if output.is_error {
            *error_message = ErrorMessage {
                message: output.message.clone(),
                span: output.error_span,
            };
            if let Some(pid) = output.preview_id {
                for mut target in preview_query.iter_mut() {
                    if target.base().preview_id == pid {
                        if let PreviewTarget::Normal {
                            control_point_overrides,
                            control_points,
                            ..
                        } = &mut *target
                        {
                            *control_point_overrides = control_points
                                .iter()
                                .flat_map(|cp| {
                                    [(&cp.x, 0), (&cp.y, 1), (&cp.z, 2)].into_iter().filter_map(
                                        |(tracked, ai)| {
                                            cp.var_names[ai]
                                                .as_ref()
                                                .map(|vn| (vn.clone(), tracked.value))
                                        },
                                    )
                                })
                                .collect();
                        }
                        break;
                    }
                }
            }
        } else {
            bevy::log::info!("CadhrLang: {}", output.message);
            *error_message = ErrorMessage::default();
        }
    }
}

const LAST_SESSION_PATH_FILE: &str = "/tmp/cadhr_last_session_path";

fn save_last_session_path(path: &Path) {
    if let Err(e) = std::fs::write(LAST_SESSION_PATH_FILE, path.to_string_lossy().as_bytes()) {
        bevy::log::warn!("Failed to save last session path: {:?}", e);
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
        previews: preview_targets.cloned().collect(),
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
    mut unsaved: ResMut<UnsavedChanges>,
) {
    for ev in ev_saved.read() {
        if ev.result.is_ok() {
            save_session(&ev.path, &editor_text, preview_query.iter());
            **current_file_path = Some(ev.path.clone());
            save_last_session_path(&ev.path);
            unsaved.dirty = false;
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
    mut ev_collision: MessageWriter<GenerateCollisionPreviewRequest>,
    mut next_preview_id: ResMut<NextPreviewId>,
    mut pending_states: ResMut<PendingPreviewStates>,
    mut free_render_layers: ResMut<FreeRenderLayers>,
    mut unsaved: ResMut<UnsavedChanges>,
) {
    for ev in ev_picked.read() {
        if let Some((db_content, previews)) = load_session(&ev.path) {
            unsaved.dirty = false;
            **editor_text = db_content;
            **current_file_path = Some(ev.path.clone());
            save_last_session_path(&ev.path);

            for (entity, target) in preview_query.iter() {
                free_render_layers.push(target.base().render_layer);
                commands.entity(entity).despawn();
            }

            pending_states.clear();

            for (i, mut preview) in previews.previews.into_iter().enumerate() {
                let base = preview.base_mut();
                if base.preview_id >= **next_preview_id {
                    **next_preview_id = base.preview_id.saturating_add(1);
                }
                base.order = i;
                let preview_id = base.preview_id;
                let query = base.query.clone();
                match &preview {
                    PreviewTarget::Normal {
                        control_point_overrides,
                        query_param_overrides,
                        ..
                    } => {
                        ev_generate.write(GeneratePreviewRequest {
                            preview_id,
                            database: (**editor_text).clone(),
                            query,
                            include_paths: (**current_file_path).iter().cloned().collect(),
                            control_point_overrides: control_point_overrides.clone(),
                            query_param_overrides: query_param_overrides.clone(),
                        });
                    }
                    PreviewTarget::Collision { .. } => {
                        ev_collision.write(GenerateCollisionPreviewRequest {
                            preview_id,
                            database: (**editor_text).clone(),
                            query,
                            include_paths: (**current_file_path).iter().cloned().collect(),
                        });
                    }
                }
                pending_states.insert(preview_id, preview);
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

pub(super) fn bom_json_saved(mut ev_saved: MessageReader<DialogFileSaved<BomJsonFileContents>>) {
    for ev in ev_saved.read() {
        if ev.result.is_ok() {
            bevy::log::info!("BOM JSON saved to: {:?}", ev.path);
        } else {
            bevy::log::error!("Failed to save BOM JSON: {:?}", ev.result);
        }
    }
}

pub(super) fn restore_last_session(
    mut editor_text: ResMut<EditorText>,
    mut current_file_path: ResMut<CurrentFilePath>,
    mut ev_generate: MessageWriter<GeneratePreviewRequest>,
    mut ev_collision: MessageWriter<GenerateCollisionPreviewRequest>,
    mut next_preview_id: ResMut<NextPreviewId>,
    mut pending_states: ResMut<PendingPreviewStates>,
    mut unsaved: ResMut<UnsavedChanges>,
) {
    let path_str = match std::fs::read_to_string(LAST_SESSION_PATH_FILE) {
        Ok(s) => s,
        Err(_) => return,
    };
    let path = std::path::PathBuf::from(path_str.trim());
    if !path.is_dir() {
        return;
    }
    if let Some((db_content, previews)) = load_session(&path) {
        **editor_text = db_content;
        **current_file_path = Some(path);
        unsaved.dirty = false;

        for (i, mut preview) in previews.previews.into_iter().enumerate() {
            let base = preview.base_mut();
            if base.preview_id >= **next_preview_id {
                **next_preview_id = base.preview_id.saturating_add(1);
            }
            base.order = i;
            let preview_id = base.preview_id;
            let query = base.query.clone();
            match &preview {
                PreviewTarget::Normal {
                    control_point_overrides,
                    query_param_overrides,
                    ..
                } => {
                    ev_generate.write(GeneratePreviewRequest {
                        preview_id,
                        database: (**editor_text).clone(),
                        query,
                        include_paths: (**current_file_path).iter().cloned().collect(),
                        control_point_overrides: control_point_overrides.clone(),
                        query_param_overrides: query_param_overrides.clone(),
                    });
                }
                PreviewTarget::Collision { .. } => {
                    ev_collision.write(GenerateCollisionPreviewRequest {
                        preview_id,
                        database: (**editor_text).clone(),
                        query,
                        include_paths: (**current_file_path).iter().cloned().collect(),
                    });
                }
            }
            pending_states.insert(preview_id, preview);
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

pub(super) fn auto_reload_system(
    mut auto_reload: ResMut<AutoReload>,
    current_file_path: Res<CurrentFilePath>,
    mut editor_text: ResMut<EditorText>,
    preview_query: Query<&PreviewTarget>,
    mut ev_generate: MessageWriter<GeneratePreviewRequest>,
    mut ev_collision: MessageWriter<GenerateCollisionPreviewRequest>,
) {
    if !auto_reload.enabled {
        return;
    }
    let Some(ref path) = **current_file_path else {
        return;
    };
    let db_path = path.join("db.cadhr");
    let Ok(metadata) = std::fs::metadata(&db_path) else {
        return;
    };
    let Ok(file_modified) = metadata.modified() else {
        return;
    };
    if auto_reload.last_modified == Some(file_modified) {
        return;
    }
    auto_reload.last_modified = Some(file_modified);
    if let Ok(content) = std::fs::read_to_string(&db_path) {
        **editor_text = content;
        for target in preview_query.iter() {
            send_update_for_target(
                target,
                &editor_text,
                &current_file_path,
                &mut ev_generate,
                &mut ev_collision,
            );
        }
    }
}
