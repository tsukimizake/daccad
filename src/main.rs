mod export;
mod highlight;
mod interpreter;
mod preview;
mod session;
mod ui;

use cadhr_lang::bom::BomEntry;
use cadhr_lang::manifold_bridge::ControlPoint;
use cadhr_lang::parse::{QueryParam, SrcSpan};
use iced::widget::{
    column, row, scrollable, shader, slider, text, text_editor, text_input, toggler,
};
use iced::{Element, Fill, Subscription, Task};
use interpreter::{CollisionJobParams, CollisionJobResult, MeshJobParams, MeshJobResult};
use preview::Scene;
use session::SessionPreview;
use std::collections::HashMap;
use std::path::PathBuf;

fn main() -> iced::Result {
    iced::application("cadhr", update, view)
        .subscription(subscription)
        .run_with(new)
}

#[derive(Clone, PartialEq)]
enum PreviewKind {
    Normal,
    Collision {
        part_count: usize,
        collision_count: usize,
    },
}

struct Preview {
    id: u64,
    kind: PreviewKind,
    query: String,
    scene: Scene,
    control_points: Vec<ControlPoint>,
    control_point_overrides: HashMap<String, f64>,
    query_params: Vec<QueryParam>,
    query_param_overrides: HashMap<String, f64>,
    /// Keep raw mesh data for export
    last_vertices: Vec<preview::pipeline::Vertex>,
    last_indices: Vec<u32>,
    bom_entries: Vec<BomEntry>,
}

struct Model {
    editor: text_editor::Content,
    previews: Vec<Preview>,
    next_preview_id: u64,
    current_file_path: Option<PathBuf>,
    error_message: String,
    error_span: Option<SrcSpan>,
    unsaved: bool,
    auto_reload: bool,
    last_modified: Option<std::time::SystemTime>,
    selected_cp: Option<(u64, usize)>, // (preview_id, cp_index)
}

#[derive(Debug, Clone)]
enum Msg {
    EditorAction(text_editor::Action),
    AddPreview,
    AddCollisionCheck,
    UpdatePreviews,
    UpdatePreview(u64),
    ClosePreview(u64),
    MovePreviewUp(u64),
    MovePreviewDown(u64),
    QueryChanged(u64, String),
    PreviewGenerated(u64, Result<MeshJobResult, String>),
    CollisionGenerated(u64, Result<CollisionJobResult, String>),
    CpOverrideChanged(u64, String, f64),
    QpOverrideChanged(u64, String, f64),
    PreviewClicked {
        preview_id: u64,
        u: f32,
        v: f32,
        rotate_x: f64,
        rotate_y: f64,
        zoom: f32,
        aspect: f32,
    },

    // File I/O
    NewSession,
    OpenSession,
    SaveSession,
    SaveSessionAs,
    SessionOpened(Option<(PathBuf, String, session::SessionPreviews)>),
    SessionSaved(Result<PathBuf, String>),

    // Export
    Export3MF(u64),
    ExportBOM(u64),

    // Auto reload
    ToggleAutoReload,
    CheckFileChanged,

    // Source edit: slider release でエディタのスパンを新値で置換
    CpSourceEdit(u64, String, cadhr_lang::parse::SrcSpan), // preview_id, var_name, span
}

fn new() -> (Model, Task<Msg>) {
    if let Some(path) = session::restore_last_session_path() {
        if let Some((db_content, previews)) = session::load_session(&path) {
            let mut model = Model {
                editor: text_editor::Content::with_text(&db_content),
                previews: vec![],
                next_preview_id: 0,
                current_file_path: Some(path),
                error_message: String::new(),
                error_span: None,
                unsaved: false,
                auto_reload: false,
                last_modified: None,
                selected_cp: None,
            };
            let mut tasks = vec![];
            for sp in previews.previews {
                let id = sp.preview_id;
                if id >= model.next_preview_id {
                    model.next_preview_id = id + 1;
                }
                model.previews.push(Preview {
                    id,
                    kind: PreviewKind::Normal,
                    query: sp.query,
                    scene: Scene::new(),
                    control_points: vec![],
                    control_point_overrides: sp.control_point_overrides,
                    query_params: vec![],
                    query_param_overrides: sp.query_param_overrides,
                    last_vertices: vec![],
                    last_indices: vec![],
                    bom_entries: vec![],
                });
                tasks.push(generate_preview(&model, id));
            }
            return (model, Task::batch(tasks));
        }
    }

    (
        Model {
            editor: text_editor::Content::with_text("main :- cube(10, 20, 30)."),
            previews: vec![],
            next_preview_id: 0,
            current_file_path: None,
            error_message: String::new(),
            error_span: None,
            unsaved: false,
            auto_reload: false,
            last_modified: None,
            selected_cp: None,
        },
        Task::none(),
    )
}

fn update(model: &mut Model, message: Msg) -> Task<Msg> {
    match message {
        Msg::EditorAction(action) => {
            let is_edit = action.is_edit();
            model.editor.perform(action);
            if is_edit {
                model.unsaved = true;
            }
            Task::none()
        }
        Msg::AddPreview => {
            let id = model.next_preview_id;
            model.next_preview_id += 1;
            model.previews.push(Preview {
                id,
                kind: PreviewKind::Normal,
                query: "main.".to_string(),
                scene: Scene::new(),
                control_points: vec![],
                control_point_overrides: Default::default(),
                query_params: vec![],
                query_param_overrides: Default::default(),
                last_vertices: vec![],
                last_indices: vec![],
                bom_entries: vec![],
            });
            model.unsaved = true;
            generate_preview(model, id)
        }
        Msg::AddCollisionCheck => {
            let id = model.next_preview_id;
            model.next_preview_id += 1;
            model.previews.push(Preview {
                id,
                kind: PreviewKind::Collision {
                    part_count: 0,
                    collision_count: 0,
                },
                query: "main.".to_string(),
                scene: {
                    let mut s = Scene::new();
                    s.color = [0.4, 0.5, 0.7, 0.7];
                    s
                },
                control_points: vec![],
                control_point_overrides: Default::default(),
                query_params: vec![],
                query_param_overrides: Default::default(),
                last_vertices: vec![],
                last_indices: vec![],
                bom_entries: vec![],
            });
            model.unsaved = true;
            generate_preview(model, id)
        }
        Msg::UpdatePreviews => {
            let ids: Vec<u64> = model.previews.iter().map(|p| p.id).collect();
            let tasks: Vec<Task<Msg>> = ids.iter().map(|&id| generate_preview(model, id)).collect();
            Task::batch(tasks)
        }
        Msg::UpdatePreview(id) => generate_preview(model, id),
        Msg::ClosePreview(id) => {
            model.previews.retain(|p| p.id != id);
            model.unsaved = true;
            Task::none()
        }
        Msg::MovePreviewUp(id) => {
            if let Some(i) = model.previews.iter().position(|p| p.id == id) {
                if i > 0 {
                    model.previews.swap(i - 1, i);
                    model.unsaved = true;
                }
            }
            Task::none()
        }
        Msg::MovePreviewDown(id) => {
            if let Some(i) = model.previews.iter().position(|p| p.id == id) {
                if i + 1 < model.previews.len() {
                    model.previews.swap(i, i + 1);
                    model.unsaved = true;
                }
            }
            Task::none()
        }
        Msg::QueryChanged(id, query) => {
            if let Some(p) = model.previews.iter_mut().find(|p| p.id == id) {
                p.query = query;
            }
            Task::none()
        }
        Msg::PreviewGenerated(id, result) => {
            match result {
                Ok(result) => {
                    if let Some((msg, span)) = &result.error {
                        model.error_message = msg.clone();
                        model.error_span = *span;
                    } else {
                        model.error_message.clear();
                        model.error_span = None;
                    }
                    if let Some(p) = model.previews.iter_mut().find(|p| p.id == id) {
                        p.last_vertices = result.vertices.clone();
                        p.last_indices = result.indices.clone();
                        p.control_points = result.control_points;
                        p.query_params = result.query_params;
                        p.bom_entries = result.bom_entries;
                        let selected = model
                            .selected_cp
                            .and_then(|(pid, ci)| if pid == id { Some(ci) } else { None });
                        p.scene.set_mesh_with_control_points(
                            result.vertices,
                            result.indices,
                            &p.control_points,
                            selected,
                        );
                    }
                }
                Err(e) => {
                    model.error_message = e;
                }
            }
            Task::none()
        }
        Msg::CollisionGenerated(id, result) => {
            match result {
                Ok(result) => {
                    if let Some((msg, span)) = &result.error {
                        model.error_message = msg.clone();
                        model.error_span = *span;
                    } else {
                        model.error_message.clear();
                        model.error_span = None;
                    }
                    if let Some(p) = model.previews.iter_mut().find(|p| p.id == id) {
                        p.last_vertices = result.vertices.clone();
                        p.last_indices = result.indices.clone();
                        p.kind = PreviewKind::Collision {
                            part_count: result.part_count,
                            collision_count: result.collision_count,
                        };
                        p.scene.set_mesh(result.vertices, result.indices);
                    }
                }
                Err(e) => {
                    model.error_message = e;
                }
            }
            Task::none()
        }
        Msg::CpOverrideChanged(id, var_name, value) => {
            if let Some(p) = model.previews.iter_mut().find(|p| p.id == id) {
                p.control_point_overrides.insert(var_name, value);
            }
            generate_preview(model, id)
        }
        Msg::QpOverrideChanged(id, name, value) => {
            if let Some(p) = model.previews.iter_mut().find(|p| p.id == id) {
                p.query_param_overrides.insert(name, value);
            }
            generate_preview(model, id)
        }
        Msg::PreviewClicked {
            preview_id,
            u,
            v,
            rotate_x,
            rotate_y,
            zoom,
            aspect,
        } => {
            if let Some(p) = model.previews.iter().find(|p| p.id == preview_id) {
                if p.control_points.is_empty() {
                    model.selected_cp = None;
                    return generate_preview(model, preview_id);
                }

                let cam = preview::CameraState::with_values(rotate_x, rotate_y, zoom);
                let (ray_origin, ray_dir) =
                    preview::generate_ray_from_uv(u, v, &cam, p.scene.base_camera_distance, aspect);

                let aabb = compute_aabb_from_vertices(&p.last_vertices);
                let hit_radius = (aabb * 0.03).max(0.5) * 1.5;

                let mut best_hit: Option<(f64, usize)> = None;
                for (ci, cp) in p.control_points.iter().enumerate() {
                    let center = [cp.x.value, cp.y.value, cp.z.value];
                    if let Some(t) = preview::ray_sphere_intersect(
                        &ray_origin,
                        &ray_dir,
                        &center,
                        hit_radius as f64,
                    ) {
                        if best_hit.is_none() || t < best_hit.unwrap().0 {
                            best_hit = Some((t, ci));
                        }
                    }
                }

                if let Some((_, ci)) = best_hit {
                    model.selected_cp = Some((preview_id, ci));
                } else {
                    model.selected_cp = None;
                }

                return generate_preview(model, preview_id);
            }
            Task::none()
        }

        // File I/O
        Msg::NewSession => {
            model.editor = text_editor::Content::with_text("main :- cube(10, 20, 30).");
            model.previews.clear();
            model.next_preview_id = 0;
            model.current_file_path = None;
            model.error_message.clear();
            model.error_span = None;
            model.unsaved = false;
            Task::none()
        }
        Msg::OpenSession => Task::perform(
            async {
                let handle = rfd::AsyncFileDialog::new()
                    .set_title("Open Session Directory")
                    .pick_folder()
                    .await?;
                let path = handle.path().to_path_buf();
                let (db, previews) = session::load_session(&path)?;
                Some((path, db, previews))
            },
            Msg::SessionOpened,
        ),
        Msg::SessionOpened(result) => {
            if let Some((path, db_content, previews)) = result {
                model.editor = text_editor::Content::with_text(&db_content);
                model.previews.clear();
                model.next_preview_id = 0;
                model.current_file_path = Some(path.clone());
                model.unsaved = false;
                session::save_last_session_path(&path);

                let mut tasks = vec![];
                for sp in previews.previews {
                    let id = sp.preview_id;
                    if id >= model.next_preview_id {
                        model.next_preview_id = id + 1;
                    }
                    model.previews.push(Preview {
                        id,
                        kind: PreviewKind::Normal,
                        query: sp.query,
                        scene: Scene::new(),
                        control_points: vec![],
                        control_point_overrides: sp.control_point_overrides,
                        query_params: vec![],
                        query_param_overrides: sp.query_param_overrides,
                        last_vertices: vec![],
                        last_indices: vec![],
                        bom_entries: vec![],
                    });
                    tasks.push(generate_preview(model, id));
                }
                return Task::batch(tasks);
            }
            Task::none()
        }
        Msg::SaveSession => {
            if let Some(ref path) = model.current_file_path {
                let path = path.clone();
                let text = model.editor.text();
                let previews = collect_session_previews(model);
                Task::perform(
                    async move { session::save_session(&path, &text, &previews).map(|()| path) },
                    Msg::SessionSaved,
                )
            } else {
                update(model, Msg::SaveSessionAs)
            }
        }
        Msg::SaveSessionAs => {
            let text = model.editor.text();
            let previews = collect_session_previews(model);
            Task::perform(
                async move {
                    let handle = rfd::AsyncFileDialog::new()
                        .set_title("Save Session Directory")
                        .set_file_name("untitled")
                        .save_file()
                        .await;
                    match handle {
                        Some(h) => {
                            let path = h.path().to_path_buf();
                            session::save_session(&path, &text, &previews).map(|()| path)
                        }
                        None => Err("Cancelled".to_string()),
                    }
                },
                Msg::SessionSaved,
            )
        }
        Msg::SessionSaved(result) => {
            match result {
                Ok(path) => {
                    session::save_last_session_path(&path);
                    model.current_file_path = Some(path);
                    model.unsaved = false;
                }
                Err(e) if e != "Cancelled" => {
                    model.error_message = format!("Save failed: {}", e);
                }
                _ => {}
            }
            Task::none()
        }

        // Export
        Msg::Export3MF(id) => {
            if let Some(p) = model.previews.iter().find(|p| p.id == id) {
                let vertices = p.last_vertices.clone();
                let indices = p.last_indices.clone();
                let query = p.query.clone();
                let base_name = model
                    .current_file_path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .and_then(|n| n.to_str())
                    .unwrap_or("untitled")
                    .to_string();
                Task::perform(
                    async move {
                        let data = export::vertices_to_threemf(&vertices, &indices);
                        if let Some(data) = data {
                            let file_name = format!("{}_{}.3mf", base_name, sanitize(&query));
                            let handle = rfd::AsyncFileDialog::new()
                                .set_title("Export 3MF")
                                .add_filter("3MF", &["3mf"])
                                .set_file_name(&file_name)
                                .save_file()
                                .await;
                            if let Some(h) = handle {
                                let _ = std::fs::write(h.path(), data);
                            }
                        }
                    },
                    |()| Msg::UpdatePreviews,
                )
            } else {
                Task::none()
            }
        }
        Msg::ExportBOM(id) => {
            if let Some(p) = model.previews.iter().find(|p| p.id == id) {
                if p.bom_entries.is_empty() {
                    return Task::none();
                }
                let json = cadhr_lang::bom::bom_entries_to_json(&p.bom_entries);
                let base_name = model
                    .current_file_path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .and_then(|n| n.to_str())
                    .unwrap_or("untitled")
                    .to_string();
                Task::perform(
                    async move {
                        let handle = rfd::AsyncFileDialog::new()
                            .set_title("Export BOM")
                            .add_filter("JSON", &["json"])
                            .set_file_name(&format!("{}_bom.json", base_name))
                            .save_file()
                            .await;
                        if let Some(h) = handle {
                            let _ = std::fs::write(h.path(), json.as_bytes());
                        }
                    },
                    |()| Msg::UpdatePreviews,
                )
            } else {
                Task::none()
            }
        }

        Msg::ToggleAutoReload => {
            model.auto_reload = !model.auto_reload;
            if model.auto_reload {
                model.last_modified = model.current_file_path.as_ref().and_then(|p| {
                    std::fs::metadata(p.join("db.cadhr"))
                        .and_then(|m| m.modified())
                        .ok()
                });
            }
            Task::none()
        }
        Msg::CheckFileChanged => {
            if !model.auto_reload {
                return Task::none();
            }
            if let Some(ref path) = model.current_file_path.clone() {
                let db_path = path.join("db.cadhr");
                if let Ok(meta) = std::fs::metadata(&db_path) {
                    if let Ok(modified) = meta.modified() {
                        if model.last_modified.is_none_or(|prev| modified > prev) {
                            model.last_modified = Some(modified);
                            if let Ok(content) = std::fs::read_to_string(&db_path) {
                                model.editor = text_editor::Content::with_text(&content);
                                return update(model, Msg::UpdatePreviews);
                            }
                        }
                    }
                }
            }
            Task::none()
        }
        Msg::CpSourceEdit(preview_id, var_name, span) => {
            if span.file_id != 0 {
                return Task::none();
            }
            let value = model
                .previews
                .iter()
                .find(|p| p.id == preview_id)
                .and_then(|p| p.control_point_overrides.get(&var_name).copied());
            let Some(value) = value else {
                return Task::none();
            };
            let text = model.editor.text();
            let start = span.start.min(text.len());
            let end = span.end.min(text.len());
            let new_text = format!(
                "{}{}{}",
                &text[..start],
                format_cp_value(value),
                &text[end..]
            );
            model.editor = text_editor::Content::with_text(&new_text);
            model.unsaved = true;
            generate_preview(model, preview_id)
        }
    }
}

fn generate_preview(model: &Model, id: u64) -> Task<Msg> {
    let Some(preview) = model.previews.iter().find(|p| p.id == id) else {
        return Task::none();
    };
    let db = model.editor.text();
    let query = preview.query.clone();
    let include_paths: Vec<_> = model.current_file_path.iter().cloned().collect();

    match &preview.kind {
        PreviewKind::Normal => {
            let params = MeshJobParams {
                database: db,
                query,
                include_paths,
                control_point_overrides: preview.control_point_overrides.clone(),
                query_param_overrides: preview.query_param_overrides.clone(),
            };
            Task::perform(
                async move {
                    std::thread::spawn(move || interpreter::run_mesh_job(params))
                        .join()
                        .map_err(|_| "Interpreter thread panicked".to_string())
                },
                move |result| Msg::PreviewGenerated(id, result),
            )
        }
        PreviewKind::Collision { .. } => {
            let params = CollisionJobParams {
                database: db,
                query,
                include_paths,
            };
            Task::perform(
                async move {
                    std::thread::spawn(move || interpreter::run_collision_job(params))
                        .join()
                        .map_err(|_| "Interpreter thread panicked".to_string())
                },
                move |result| Msg::CollisionGenerated(id, result),
            )
        }
    }
}

fn collect_session_previews(model: &Model) -> Vec<SessionPreview> {
    model
        .previews
        .iter()
        .enumerate()
        .map(|(i, p)| SessionPreview {
            preview_id: p.id,
            query: p.query.clone(),
            zoom: 10.0, // TODO: read from camera state
            rotate_x: 0.0,
            rotate_y: 0.0,
            order: i,
            control_point_overrides: p.control_point_overrides.clone(),
            query_param_overrides: p.query_param_overrides.clone(),
        })
        .collect()
}

fn subscription(model: &Model) -> Subscription<Msg> {
    if model.auto_reload {
        iced::time::every(std::time::Duration::from_secs(1)).map(|_| Msg::CheckFileChanged)
    } else {
        Subscription::none()
    }
}

fn view(model: &Model) -> Element<'_, Msg> {
    let title = model
        .current_file_path
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("untitled");
    let dirty_marker = if model.unsaved { " *" } else { "" };

    let toolbar = row![
        ui::parts::dark_button("New").on_press(Msg::NewSession),
        ui::parts::dark_button("Open").on_press(Msg::OpenSession),
        ui::parts::dark_button("Save").on_press(Msg::SaveSession),
        ui::parts::dark_button("Save As").on_press(Msg::SaveSessionAs),
        text(" | "),
        ui::parts::dark_button("Add Preview").on_press(Msg::AddPreview),
        ui::parts::dark_button("Collision Check")
            .on_press(Msg::AddCollisionCheck),
        ui::parts::dark_button("Update All")
            .on_press(Msg::UpdatePreviews),
        text(" | "),
        toggler(model.auto_reload)
            .label("Auto Reload")
            .on_toggle(|_| Msg::ToggleAutoReload),
        text(format!("  {}{}", title, dirty_marker)),
    ]
    .spacing(4)
    .padding(4);

    let hl_settings = highlight::Settings {
        error_span: model.error_span,
        has_error: !model.error_message.is_empty(),
    };
    let editor = text_editor(&model.editor)
        .on_action(Msg::EditorAction)
        .key_binding(ui::parts::emacs_key_binding)
        .highlight_with::<highlight::SpanHighlighter>(hl_settings, highlight::format)
        .height(Fill);

    let preview_list: Element<'_, Msg> = if model.previews.is_empty() {
        text("Add Preview を押してください").into()
    } else {
        let total = model.previews.len();
        let items: Vec<Element<'_, Msg>> = model
            .previews
            .iter()
            .enumerate()
            .map(|(i, p)| view_preview(p, i, total))
            .collect();
        scrollable(column(items).spacing(12)).height(Fill).into()
    };

    let error_bar: Element<'_, Msg> = if model.error_message.is_empty() {
        column![].into()
    } else {
        text(&model.error_message)
            .color(iced::Color::from_rgb(1.0, 0.3, 0.3))
            .into()
    };

    column![
        toolbar,
        row![editor, preview_list].spacing(4).height(Fill),
        error_bar,
    ]
    .spacing(4)
    .padding(4)
    .into()
}

fn view_preview<'a>(p: &'a Preview, index: usize, total: usize) -> Element<'a, Msg> {
    let id = p.id;

    let preview_label = match &p.kind {
        PreviewKind::Normal => format!("Preview {}", id),
        PreviewKind::Collision {
            part_count,
            collision_count,
        } => {
            if *collision_count > 0 {
                format!(
                    "Collision {} — {} parts, {} collision(s) ⚠",
                    id, part_count, collision_count
                )
            } else {
                format!("Collision {} — {} parts, no collisions ✓", id, part_count)
            }
        }
    };
    let up_btn = if index > 0 {
        ui::parts::dark_button("↑").on_press(Msg::MovePreviewUp(id))
    } else {
        ui::parts::dark_button("↑")
    };
    let down_btn = if index + 1 < total {
        ui::parts::dark_button("↓").on_press(Msg::MovePreviewDown(id))
    } else {
        ui::parts::dark_button("↓")
    };
    let mut header = row![
        up_btn,
        down_btn,
        text(preview_label),
        ui::parts::dark_button("Update")
            .on_press(Msg::UpdatePreview(id)),
        ui::parts::dark_button("Export 3MF")
            .on_press(Msg::Export3MF(id)),
    ]
    .spacing(4);
    if !p.bom_entries.is_empty() {
        header = header.push(ui::parts::dark_button("Export BOM").on_press(Msg::ExportBOM(id)));
    }
    let header = header.push(ui::parts::dark_button("Close").on_press(Msg::ClosePreview(id)));

    let query_row = row![
        text("?- "),
        text_input("query", &p.query)
            .on_input(move |q| Msg::QueryChanged(id, q))
            .on_submit(Msg::UpdatePreview(id)),
    ]
    .spacing(4);

    let shader_view: Element<'a, Msg> = Element::from(shader(&p.scene).width(Fill).height(300))
        .map(move |msg| match msg {
            preview::SceneMessage::Clicked {
                u,
                v,
                rotate_x,
                rotate_y,
                zoom,
                aspect,
            } => Msg::PreviewClicked {
                preview_id: id,
                u,
                v,
                rotate_x,
                rotate_y,
                zoom,
                aspect,
            },
        });

    let qp_items = p.query_params.iter().map(|qp| {
        let name = qp.name.clone();
        let (min_val, max_val) = query_param_range(qp);
        let current = p
            .query_param_overrides
            .get(&qp.name)
            .copied()
            .unwrap_or_else(|| {
                qp.default_value
                    .map(|dv| dv.to_f64())
                    .unwrap_or((min_val + max_val) / 2.0)
            });
        let qp_id = id;
        let qp_name = name.clone();
        row![
            text(format!("{}:", name)).width(80),
            slider(min_val..=max_val, current, move |v| {
                Msg::QpOverrideChanged(qp_id, qp_name.clone(), v)
            })
            .step(0.1)
            .width(Fill),
            text(format!("{:.1}", current)).width(60),
        ]
        .spacing(4)
        .into()
    });

    let cp_items = p.control_points.iter().enumerate().map(|(ci, cp)| {
        let label = cp
            .name
            .as_deref()
            .map(|n| format!("CP {}", n))
            .unwrap_or_else(|| format!("CP {}", ci));

        let axis_elements: Vec<Element<'a, Msg>> = [("X", &cp.x), ("Y", &cp.y), ("Z", &cp.z)]
            .iter()
            .enumerate()
            .flat_map(|(axis_idx, (axis_label, tracked))| {
                let val = cp.var_names[axis_idx]
                    .as_ref()
                    .and_then(|vn| p.control_point_overrides.get(vn).copied())
                    .unwrap_or(tracked.value);

                let slider_el: Option<Element<'a, Msg>> =
                    cp.var_names[axis_idx].as_ref().map(|var_name| {
                        let cp_id = id;
                        let vn = var_name.clone();
                        let source_span = tracked.source_span;
                        let range_half = (val.abs() + 50.0).max(50.0);
                        let mut sl =
                            slider((val - range_half)..=(val + range_half), val, move |v| {
                                Msg::CpOverrideChanged(cp_id, vn.clone(), v)
                            })
                            .step(0.5)
                            .width(80);
                        if let Some(span) = source_span.filter(|s| s.file_id == 0) {
                            let vn2 = var_name.clone();
                            sl = sl.on_release(Msg::CpSourceEdit(id, vn2, span));
                        }
                        sl.into()
                    });

                [
                    Some(text(*axis_label).into()),
                    slider_el,
                    Some(text(format!("{:.1}", val)).width(50).into()),
                ]
                .into_iter()
                .flatten()
            })
            .collect();

        let label_el: Element<'a, Msg> = text(label).width(80).into();
        row(std::iter::once(label_el).chain(axis_elements))
            .spacing(4)
            .into()
    });

    column(
        [header.into(), query_row.into(), shader_view.into()]
            .into_iter()
            .chain(qp_items)
            .chain(cp_items),
    )
    .spacing(4)
    .into()
}

fn compute_aabb_from_vertices(vertices: &[preview::pipeline::Vertex]) -> f32 {
    let mut max_extent: f32 = 0.0;
    for v in vertices {
        for &c in &v.position {
            max_extent = max_extent.max(c.abs());
        }
    }
    max_extent
}

fn query_param_range(qp: &QueryParam) -> (f64, f64) {
    let min = qp.min.as_ref().map(|b| b.value.to_f64()).unwrap_or(-100.0);
    let max = qp.max.as_ref().map(|b| b.value.to_f64()).unwrap_or(100.0);
    (min, max)
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

/// CP値をソースコードに書き戻す際のフォーマット。整数なら小数点なし、小数なら末尾ゼロを除去。
fn format_cp_value(value: f64) -> String {
    let s = format!("{:.6}", value);
    let s = s.trim_end_matches('0');
    let s = s.trim_end_matches('.');
    s.to_string()
}

