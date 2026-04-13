mod export;
mod highlight;
mod interpreter;
mod preview;
mod session;

use cadhr_lang::manifold_bridge::ControlPoint;
use cadhr_lang::parse::{QueryParam, SrcSpan};
use iced::widget::{button, column, row, scrollable, shader, slider, text, text_editor, text_input, toggler};
use iced::{Element, Fill, Subscription, Task};
use interpreter::{MeshJobParams, MeshJobResult};
use preview::Scene;
use session::SessionPreview;
use std::collections::HashMap;
use std::path::PathBuf;

fn main() -> iced::Result {
    iced::application("cadhr", App::update, App::view)
        .subscription(App::subscription)
        .run_with(App::new)
}

struct Preview {
    id: u64,
    query: String,
    scene: Scene,
    control_points: Vec<ControlPoint>,
    control_point_overrides: HashMap<String, f64>,
    query_params: Vec<QueryParam>,
    query_param_overrides: HashMap<String, f64>,
    /// Keep raw mesh data for export
    last_vertices: Vec<preview::pipeline::Vertex>,
    last_indices: Vec<u32>,
}

struct App {
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
enum Message {
    EditorAction(text_editor::Action),
    AddPreview,
    UpdatePreviews,
    UpdatePreview(u64),
    ClosePreview(u64),
    QueryChanged(u64, String),
    PreviewGenerated(u64, Result<MeshJobResult, String>),
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

impl App {
    fn new() -> (Self, Task<Message>) {
        // Try to restore last session
        if let Some(path) = session::restore_last_session_path() {
            if let Some((db_content, previews)) = session::load_session(&path) {
                let mut app = Self {
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
                    if id >= app.next_preview_id {
                        app.next_preview_id = id + 1;
                    }
                    app.previews.push(Preview {
                        id,
                        query: sp.query,
                        scene: Scene::new(),
                        control_points: vec![],
                        control_point_overrides: sp.control_point_overrides,
                        query_params: vec![],
                        query_param_overrides: sp.query_param_overrides,
                        last_vertices: vec![],
                        last_indices: vec![],
                    });
                    tasks.push(app.generate_preview(id));
                }
                return (app, Task::batch(tasks));
            }
        }

        (
            Self {
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

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::EditorAction(action) => {
                let is_edit = action.is_edit();
                self.editor.perform(action);
                if is_edit {
                    self.unsaved = true;
                }
                Task::none()
            }
            Message::AddPreview => {
                let id = self.next_preview_id;
                self.next_preview_id += 1;
                self.previews.push(Preview {
                    id,
                    query: "main.".to_string(),
                    scene: Scene::new(),
                    control_points: vec![],
                    control_point_overrides: Default::default(),
                    query_params: vec![],
                    query_param_overrides: Default::default(),
                    last_vertices: vec![],
                    last_indices: vec![],
                });
                self.unsaved = true;
                self.generate_preview(id)
            }
            Message::UpdatePreviews => {
                let tasks: Vec<Task<Message>> =
                    self.previews.iter().map(|p| self.generate_preview(p.id)).collect();
                Task::batch(tasks)
            }
            Message::UpdatePreview(id) => self.generate_preview(id),
            Message::ClosePreview(id) => {
                self.previews.retain(|p| p.id != id);
                self.unsaved = true;
                Task::none()
            }
            Message::QueryChanged(id, query) => {
                if let Some(p) = self.previews.iter_mut().find(|p| p.id == id) {
                    p.query = query;
                }
                Task::none()
            }
            Message::PreviewGenerated(id, result) => {
                match result {
                    Ok(result) => {
                        if let Some((msg, span)) = &result.error {
                            self.error_message = msg.clone();
                            self.error_span = *span;
                        } else {
                            self.error_message.clear();
                            self.error_span = None;
                        }
                        if let Some(p) = self.previews.iter_mut().find(|p| p.id == id) {
                            p.last_vertices = result.vertices.clone();
                            p.last_indices = result.indices.clone();
                            p.control_points = result.control_points;
                            p.query_params = result.query_params;
                            let selected = self.selected_cp
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
                        self.error_message = e;
                    }
                }
                Task::none()
            }
            Message::CpOverrideChanged(id, var_name, value) => {
                if let Some(p) = self.previews.iter_mut().find(|p| p.id == id) {
                    p.control_point_overrides.insert(var_name, value);
                }
                self.generate_preview(id)
            }
            Message::QpOverrideChanged(id, name, value) => {
                if let Some(p) = self.previews.iter_mut().find(|p| p.id == id) {
                    p.query_param_overrides.insert(name, value);
                }
                self.generate_preview(id)
            }
            Message::PreviewClicked {
                preview_id,
                u,
                v,
                rotate_x,
                rotate_y,
                zoom,
                aspect,
            } => {
                if let Some(p) = self.previews.iter().find(|p| p.id == preview_id) {
                    if p.control_points.is_empty() {
                        self.selected_cp = None;
                        return self.generate_preview(preview_id);
                    }

                    let cam = preview::CameraState::with_values(rotate_x, rotate_y, zoom);
                    let (ray_origin, ray_dir) = preview::generate_ray_from_uv(
                        u,
                        v,
                        &cam,
                        p.scene.base_camera_distance,
                        aspect,
                    );

                    let aabb = compute_aabb_from_vertices(&p.last_vertices);
                    let hit_radius = (aabb * 0.03).max(0.5) * 1.5;

                    let mut best_hit: Option<(f64, usize)> = None;
                    for (ci, cp) in p.control_points.iter().enumerate() {
                        let center = [cp.x.value, cp.y.value, cp.z.value];
                        if let Some(t) =
                            preview::ray_sphere_intersect(&ray_origin, &ray_dir, &center, hit_radius as f64)
                        {
                            if best_hit.is_none() || t < best_hit.unwrap().0 {
                                best_hit = Some((t, ci));
                            }
                        }
                    }

                    if let Some((_, ci)) = best_hit {
                        self.selected_cp = Some((preview_id, ci));
                    } else {
                        self.selected_cp = None;
                    }

                    return self.generate_preview(preview_id);
                }
                Task::none()
            }

            // File I/O
            Message::NewSession => {
                self.editor = text_editor::Content::with_text("main :- cube(10, 20, 30).");
                self.previews.clear();
                self.next_preview_id = 0;
                self.current_file_path = None;
                self.error_message.clear();
                self.error_span = None;
                self.unsaved = false;
                Task::none()
            }
            Message::OpenSession => {
                Task::perform(
                    async {
                        let handle = rfd::AsyncFileDialog::new()
                            .set_title("Open Session Directory")
                            .pick_folder()
                            .await?;
                        let path = handle.path().to_path_buf();
                        let (db, previews) = session::load_session(&path)?;
                        Some((path, db, previews))
                    },
                    Message::SessionOpened,
                )
            }
            Message::SessionOpened(result) => {
                if let Some((path, db_content, previews)) = result {
                    self.editor = text_editor::Content::with_text(&db_content);
                    self.previews.clear();
                    self.next_preview_id = 0;
                    self.current_file_path = Some(path.clone());
                    self.unsaved = false;
                    session::save_last_session_path(&path);

                    let mut tasks = vec![];
                    for sp in previews.previews {
                        let id = sp.preview_id;
                        if id >= self.next_preview_id {
                            self.next_preview_id = id + 1;
                        }
                        self.previews.push(Preview {
                            id,
                            query: sp.query,
                            scene: Scene::new(),
                            control_points: vec![],
                            control_point_overrides: sp.control_point_overrides,
                            query_params: vec![],
                            query_param_overrides: sp.query_param_overrides,
                            last_vertices: vec![],
                            last_indices: vec![],
                        });
                        tasks.push(self.generate_preview(id));
                    }
                    return Task::batch(tasks);
                }
                Task::none()
            }
            Message::SaveSession => {
                if let Some(ref path) = self.current_file_path {
                    let path = path.clone();
                    let text = self.editor.text();
                    let previews = self.collect_session_previews();
                    Task::perform(
                        async move {
                            session::save_session(&path, &text, &previews)
                                .map(|()| path)
                        },
                        Message::SessionSaved,
                    )
                } else {
                    self.update(Message::SaveSessionAs)
                }
            }
            Message::SaveSessionAs => {
                let text = self.editor.text();
                let previews = self.collect_session_previews();
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
                                session::save_session(&path, &text, &previews)
                                    .map(|()| path)
                            }
                            None => Err("Cancelled".to_string()),
                        }
                    },
                    Message::SessionSaved,
                )
            }
            Message::SessionSaved(result) => {
                match result {
                    Ok(path) => {
                        session::save_last_session_path(&path);
                        self.current_file_path = Some(path);
                        self.unsaved = false;
                    }
                    Err(e) if e != "Cancelled" => {
                        self.error_message = format!("Save failed: {}", e);
                    }
                    _ => {}
                }
                Task::none()
            }

            // Export
            Message::Export3MF(id) => {
                if let Some(p) = self.previews.iter().find(|p| p.id == id) {
                    let vertices = p.last_vertices.clone();
                    let indices = p.last_indices.clone();
                    let query = p.query.clone();
                    let base_name = self
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
                        |()| Message::UpdatePreviews, // no-op message after export
                    )
                } else {
                    Task::none()
                }
            }
            Message::ExportBOM(id) => {
                if let Some(p) = self.previews.iter().find(|p| p.id == id) {
                    // BOM data comes from the last interpreter result
                    // For now just skip if no data
                    let _ = id;
                }
                Task::none()
            }

            Message::ToggleAutoReload => {
                self.auto_reload = !self.auto_reload;
                if self.auto_reload {
                    self.last_modified = self.current_file_path.as_ref().and_then(|p| {
                        std::fs::metadata(p.join("db.cadhr"))
                            .and_then(|m| m.modified())
                            .ok()
                    });
                }
                Task::none()
            }
            Message::CheckFileChanged => {
                if !self.auto_reload {
                    return Task::none();
                }
                if let Some(ref path) = self.current_file_path {
                    let db_path = path.join("db.cadhr");
                    if let Ok(meta) = std::fs::metadata(&db_path) {
                        if let Ok(modified) = meta.modified() {
                            if self.last_modified.is_none_or(|prev| modified > prev) {
                                self.last_modified = Some(modified);
                                if let Ok(content) = std::fs::read_to_string(&db_path) {
                                    self.editor = text_editor::Content::with_text(&content);
                                    return self.update(Message::UpdatePreviews);
                                }
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::CpSourceEdit(preview_id, var_name, span) => {
                // メインDBファイル (file_id=0) のスパンのみ編集可能
                if span.file_id != 0 {
                    return Task::none();
                }
                let value = self
                    .previews
                    .iter()
                    .find(|p| p.id == preview_id)
                    .and_then(|p| p.control_point_overrides.get(&var_name).copied());
                let Some(value) = value else {
                    return Task::none();
                };
                let text = self.editor.text();
                // span の終端が改行 \n を含む場合があるので clamp
                let start = span.start.min(text.len());
                let end = span.end.min(text.len());
                let new_text = format!(
                    "{}{}{}",
                    &text[..start],
                    format_cp_value(value),
                    &text[end..]
                );
                self.editor = text_editor::Content::with_text(&new_text);
                self.unsaved = true;
                self.generate_preview(preview_id)
            }
        }
    }

    fn generate_preview(&self, id: u64) -> Task<Message> {
        let Some(preview) = self.previews.iter().find(|p| p.id == id) else {
            return Task::none();
        };
        let params = MeshJobParams {
            database: self.editor.text(),
            query: preview.query.clone(),
            include_paths: self.current_file_path.iter().cloned().collect(),
            control_point_overrides: preview.control_point_overrides.clone(),
            query_param_overrides: preview.query_param_overrides.clone(),
        };
        Task::perform(
            async move {
                std::thread::spawn(move || interpreter::run_mesh_job(params))
                    .join()
                    .map_err(|_| "Interpreter thread panicked".to_string())
            },
            move |result| Message::PreviewGenerated(id, result),
        )
    }

    fn collect_session_previews(&self) -> Vec<SessionPreview> {
        self.previews
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

    fn subscription(&self) -> Subscription<Message> {
        if self.auto_reload {
            iced::time::every(std::time::Duration::from_secs(1))
                .map(|_| Message::CheckFileChanged)
        } else {
            Subscription::none()
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let title = self
            .current_file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("untitled");
        let dirty_marker = if self.unsaved { " *" } else { "" };

        let toolbar = row![
            button("New").on_press(Message::NewSession),
            button("Open").on_press(Message::OpenSession),
            button("Save").on_press(Message::SaveSession),
            button("Save As").on_press(Message::SaveSessionAs),
            text(" | "),
            button("Add Preview").on_press(Message::AddPreview),
            button("Update All").on_press(Message::UpdatePreviews),
            text(" | "),
            toggler(self.auto_reload)
                .label("Auto Reload")
                .on_toggle(|_| Message::ToggleAutoReload),
            text(format!("  {}{}", title, dirty_marker)),
        ]
        .spacing(4)
        .padding(4);

        let hl_settings = highlight::Settings {
            error_span: self.error_span,
            has_error: !self.error_message.is_empty(),
        };
        let editor = text_editor(&self.editor)
            .on_action(Message::EditorAction)
            .highlight_with::<highlight::SpanHighlighter>(hl_settings, highlight::format)
            .height(Fill);

        let preview_list: Element<'_, Message> = if self.previews.is_empty() {
            text("Add Preview を押してください").into()
        } else {
            let items: Vec<Element<'_, Message>> = self
                .previews
                .iter()
                .map(|p| self.view_preview(p))
                .collect();
            scrollable(column(items).spacing(12)).height(Fill).into()
        };

        let error_bar: Element<'_, Message> = if self.error_message.is_empty() {
            column![].into()
        } else {
            text(&self.error_message)
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

    fn view_preview<'a>(&'a self, p: &'a Preview) -> Element<'a, Message> {
        let id = p.id;

        let header = row![
            text(format!("Preview {}", id)),
            button("Update").on_press(Message::UpdatePreview(id)),
            button("Export 3MF").on_press(Message::Export3MF(id)),
            button("Close").on_press(Message::ClosePreview(id)),
        ]
        .spacing(4);

        let query_row = row![
            text("?- "),
            text_input("query", &p.query)
                .on_input(move |q| Message::QueryChanged(id, q))
                .on_submit(Message::UpdatePreview(id)),
        ]
        .spacing(4);

        let shader_view: Element<'a, preview::SceneMessage> = shader(&p.scene)
            .width(Fill)
            .height(300)
            .into();
        let shader_view: Element<'a, Message> = shader_view.map(move |msg| match msg {
            preview::SceneMessage::Clicked {
                u,
                v,
                rotate_x,
                rotate_y,
                zoom,
                aspect,
            } => Message::PreviewClicked {
                preview_id: id,
                u,
                v,
                rotate_x,
                rotate_y,
                zoom,
                aspect,
            },
        });

        let mut items: Vec<Element<'a, Message>> =
            vec![header.into(), query_row.into(), shader_view.into()];

        // Query param sliders
        for qp in &p.query_params {
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
            items.push(
                row![
                    text(format!("{}:", name)).width(80),
                    slider(min_val..=max_val, current, move |v| {
                        Message::QpOverrideChanged(qp_id, qp_name.clone(), v)
                    })
                    .step(0.1)
                    .width(Fill),
                    text(format!("{:.1}", current)).width(60),
                ]
                .spacing(4)
                .into(),
            );
        }

        // Control point values
        for (ci, cp) in p.control_points.iter().enumerate() {
            let label = cp
                .name
                .as_deref()
                .map(|n| format!("CP {}", n))
                .unwrap_or_else(|| format!("CP {}", ci));

            let mut axis_items: Vec<Element<'a, Message>> = vec![text(label).width(80).into()];

            for (axis_idx, (axis_label, tracked)) in
                [("X", &cp.x), ("Y", &cp.y), ("Z", &cp.z)]
                    .iter()
                    .enumerate()
            {
                let val = cp.var_names[axis_idx]
                    .as_ref()
                    .and_then(|vn| p.control_point_overrides.get(vn).copied())
                    .unwrap_or(tracked.value);

                axis_items.push(text(*axis_label).into());

                if let Some(ref var_name) = cp.var_names[axis_idx] {
                    let cp_id = id;
                    let vn = var_name.clone();
                    let source_span = tracked.source_span;
                    let range_half = (val.abs() + 50.0).max(50.0);
                    let mut sl = slider(
                        (val - range_half)..=(val + range_half),
                        val,
                        move |v| Message::CpOverrideChanged(cp_id, vn.clone(), v),
                    )
                    .step(0.5)
                    .width(80);
                    if let Some(span) = source_span.filter(|s| s.file_id == 0) {
                        let vn2 = var_name.clone();
                        sl = sl.on_release(Message::CpSourceEdit(id, vn2, span));
                    }
                    axis_items.push(sl.into());
                }
                axis_items.push(text(format!("{:.1}", val)).width(50).into());
            }

            items.push(row(axis_items).spacing(4).into());
        }

        column(items).spacing(4).into()
    }
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
    let min = qp
        .min
        .as_ref()
        .map(|b| b.value.to_f64())
        .unwrap_or(-100.0);
    let max = qp
        .max
        .as_ref()
        .map(|b| b.value.to_f64())
        .unwrap_or(100.0);
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
