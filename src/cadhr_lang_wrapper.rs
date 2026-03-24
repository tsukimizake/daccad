use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future};
use bevy::{mesh::Indices, prelude::*};

use crate::events::{
    CadhrLangOutput, CollisionPreviewGenerated, GenerateCollisionPreviewRequest,
    GeneratePreviewRequest, PreviewGenerated,
};
use cadhr_lang::bom::BomExtractor;
use cadhr_lang::manifold_bridge::{MeshGenerator, extract_control_points};
use cadhr_lang::module::resolve_modules;
use cadhr_lang::parse::{
    FileRegistry, SrcSpan, collect_query_params, database, parse_error_span, query as parse_query,
    substitute_query_params,
};
use cadhr_lang::term_processor::TermProcessor;
use cadhr_lang::term_rewrite::{CadhrError, execute, infer_query_param_ranges};
use manifold_rs::Mesh as RsMesh;

fn format_error(
    label: &str,
    msg: &str,
    span: Option<SrcSpan>,
    registry: &FileRegistry,
) -> (String, Option<SrcSpan>) {
    let location = span
        .map(|s| registry.format_span(&s))
        .unwrap_or_default();
    let formatted = if location.is_empty() {
        format!("{}: {}", label, msg)
    } else {
        format!("{} at {}: {}", label, location, msg)
    };
    (formatted, span)
}

pub struct CadhrLangPlugin;

impl Plugin for CadhrLangPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MeshTasks>()
            .init_resource::<CollisionTasks>()
            .add_systems(Update, (handle_mesh_tasks, handle_collision_tasks));
    }
}

struct MeshJobResult {
    outputs: Vec<CadhrLangOutput>,
    preview: PreviewGenerated,
}

struct CollisionJobResult {
    outputs: Vec<CadhrLangOutput>,
    preview: Option<CollisionPreviewGenerated>,
}

#[derive(Resource, Default)]
struct MeshTasks(Vec<Task<MeshJobResult>>);

#[derive(Resource, Default)]
struct CollisionTasks(Vec<Task<CollisionJobResult>>);

fn handle_mesh_tasks(
    mut ev_requests: MessageReader<GeneratePreviewRequest>,
    mut tasks: ResMut<MeshTasks>,
    mut ev_output: MessageWriter<CadhrLangOutput>,
    mut ev_preview: MessageWriter<PreviewGenerated>,
) {
    let pool = AsyncComputeTaskPool::get();
    for req in ev_requests.read() {
        let req = req.clone();
        tasks.0.push(pool.spawn(async move { run_mesh_job(req) }));
    }
    tasks.0.retain_mut(|task| {
        if let Some(result) = block_on(future::poll_once(task)) {
            for output in result.outputs {
                ev_output.write(output);
            }
            ev_preview.write(result.preview);
            false
        } else {
            true
        }
    });
}

fn handle_collision_tasks(
    mut ev_requests: MessageReader<GenerateCollisionPreviewRequest>,
    mut tasks: ResMut<CollisionTasks>,
    mut ev_output: MessageWriter<CadhrLangOutput>,
    mut ev_preview: MessageWriter<CollisionPreviewGenerated>,
) {
    let pool = AsyncComputeTaskPool::get();
    for req in ev_requests.read() {
        let req = req.clone();
        tasks.0.push(pool.spawn(async move { run_collision_job(req) }));
    }
    tasks.0.retain_mut(|task| {
        if let Some(result) = block_on(future::poll_once(task)) {
            for output in result.outputs {
                ev_output.write(output);
            }
            if let Some(preview) = result.preview {
                ev_preview.write(preview);
            }
            false
        } else {
            true
        }
    });
}

fn empty_mesh() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
}

fn run_mesh_job(req: GeneratePreviewRequest) -> MeshJobResult {
    let preview_id = req.preview_id;
    let db_src = req.database;
    let query = req.query;

    let mut outputs = Vec::new();
    let mut logs: Vec<String> = Vec::new();

    let resolve_result = (|| -> Result<(Vec<cadhr_lang::parse::ScopedTerm>, Vec<cadhr_lang::manifold_bridge::ControlPoint>, Vec<cadhr_lang::parse::QueryParam>), (String, Option<SrcSpan>)> {
        let mut file_registry = FileRegistry::new();
        file_registry.register_main("db".to_string(), db_src.clone());
        let query_file_id = file_registry.register("query".to_string(), query.clone());

        let (_, query_terms) =
            parse_query(&query).map_err(|e| {
                let span = parse_error_span(&query, &e)
                    .map(|mut s| { s.file_id = query_file_id; s });
                format_error("Parse error", &format!("{:?}", e), span, &file_registry)
            })?;
        let db =
            database(&db_src).map_err(|e| {
                let span = parse_error_span(&db_src, &e);
                format_error("Parse error", &format!("{:?}", e), span, &file_registry)
            })?;
        let mut db = resolve_modules(
            db,
            &req.include_paths,
            &mut std::collections::HashSet::new(),
            &mut file_registry,
        )
        .map_err(|e| (format!("Module error: {}", e), None))?;

        let mut query_params = collect_query_params(&query_terms);
        infer_query_param_ranges(&query_terms, &db, &mut query_params)
            .map_err(|e| (format!("Range inference error: {}", e), None))?;

        let mut values = req.query_param_overrides.clone();
        for param in &query_params {
            if !values.contains_key(&param.name) {
                let default = if let Some(dv) = param.default_value {
                    dv.to_f64()
                } else {
                    match (param.min.as_ref(), param.max.as_ref()) {
                        (Some(min), Some(max)) => {
                            (min.value.to_f64() + max.value.to_f64()) / 2.0
                        }
                        _ => 0.0,
                    }
                };
                values.insert(param.name.clone(), default);
            }
        }

        let substituted = substitute_query_params(&query_terms, &values);
        logs.push(format!("Query terms: {:?}", substituted));
        logs.push(format!("Database clauses: {:#?}", db));
        let (mut resolved, env) =
            execute(&mut db, substituted).map_err(|e| {
                format_error("Rewrite error", &e.to_string(), e.span(), &file_registry)
            })?;
        logs.push(format!("Resolved terms: {:?}", resolved));

        let control_points = extract_control_points(&mut resolved, &req.control_point_overrides);
        env.update_query_param_ranges(&mut query_params);
        Ok((resolved, control_points, query_params))
    })();

    let log_message = logs.join("\n");
    if !log_message.is_empty() {
        outputs.push(CadhrLangOutput {
            preview_id: None,
            message: log_message,
            is_error: false,
            error_span: None,
        });
    }

    let mesh_result = resolve_result.and_then(|(resolved, control_points, query_params)| {
        let bom_entries = BomExtractor
            .process(&resolved)
            .unwrap_or_else(|e| {
                bevy::log::warn!("BOM extraction warning: {}", e);
                vec![]
            });

        if resolved.is_empty() {
            return Ok((empty_mesh(), vec![], control_points, bom_entries, query_params));
        }

        let mesh_generator = MeshGenerator {
            include_paths: req.include_paths.clone(),
        };
        let (rs_mesh, evaluated_nodes) = mesh_generator
            .process(&resolved)
            .map_err(|e| {
                let span = e.span();
                (format!("Mesh error: {}", e), span)
            })?;
        let mesh = rs_mesh_to_bevy_mesh(&rs_mesh).map_err(|e| (e, None))?;
        Ok((mesh, evaluated_nodes, control_points, bom_entries, query_params))
    });

    let preview = match mesh_result {
        Ok((mesh, evaluated_nodes, control_points, bom_entries, query_params)) => {
            PreviewGenerated {
                preview_id,
                query,
                mesh,
                evaluated_nodes,
                control_points,
                bom_entries,
                query_params,
            }
        }
        Err((e, span)) => {
            bevy::log::error!("Failed to generate mesh: {}", e);
            outputs.push(CadhrLangOutput {
                preview_id: Some(preview_id),
                message: e,
                is_error: true,
                error_span: span,
            });
            PreviewGenerated {
                preview_id,
                query,
                mesh: empty_mesh(),
                evaluated_nodes: vec![],
                control_points: vec![],
                bom_entries: vec![],
                query_params: vec![],
            }
        }
    };

    MeshJobResult { outputs, preview }
}

fn run_collision_job(req: GenerateCollisionPreviewRequest) -> CollisionJobResult {
    let preview_id = req.preview_id;
    let db_src = req.database;
    let query_str = req.query;

    let mut outputs = Vec::new();

    let result = (|| -> Result<
        (Vec<cadhr_lang::manifold_bridge::Model3D>, Vec<std::path::PathBuf>),
        (String, Option<cadhr_lang::parse::SrcSpan>),
    > {
        let mut file_registry = FileRegistry::new();
        file_registry.register_main("db".to_string(), db_src.clone());
        let query_file_id = file_registry.register("query".to_string(), query_str.clone());

        let (_, query_terms) = parse_query(&query_str).map_err(|e| {
            let span = parse_error_span(&query_str, &e)
                .map(|mut s| { s.file_id = query_file_id; s });
            format_error("Parse error", &format!("{:?}", e), span, &file_registry)
        })?;
        let db = database(&db_src).map_err(|e| {
            let span = parse_error_span(&db_src, &e);
            format_error("Parse error", &format!("{:?}", e), span, &file_registry)
        })?;
        let mut db = resolve_modules(
            db,
            &req.include_paths,
            &mut std::collections::HashSet::new(),
            &mut file_registry,
        )
        .map_err(|e| (format!("Module error: {}", e), None))?;
        let (resolved, _) = execute(&mut db, query_terms).map_err(|e| {
            format_error("Rewrite error", &e.to_string(), e.span(), &file_registry)
        })?;

        use cadhr_lang::manifold_bridge::{ConversionError, Model3D};
        let exprs: Vec<Model3D> = resolved
            .iter()
            .filter_map(|t| match Model3D::from_term(t) {
                Ok(e) => Some(Ok(e)),
                Err(ConversionError::UnknownPrimitive(_)) => None,
                Err(e) => Some(Err(e)),
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| (format!("Conversion error: {}", e), e.span()))?;
        Ok((exprs, req.include_paths.clone()))
    })();

    let (exprs, include_paths) = match result {
        Ok(pair) => pair,
        Err((e, span)) => {
            outputs.push(CadhrLangOutput {
                preview_id: Some(preview_id),
                message: e,
                is_error: true,
                error_span: span,
            });
            return CollisionJobResult { outputs, preview: None };
        }
    };

    use cadhr_lang::collision::check_collisions;
    let converted = check_collisions(&exprs, &include_paths)
        .map_err(|e| {
            let span = e.span();
            (format!("Collision check error: {}", e), span)
        })
        .and_then(|cr| {
            let part_count = cr.part_count;
            let combined =
                rs_mesh_to_bevy_mesh(&cr.combined_mesh).map_err(|e| (e, None))?;
            let collisions: Vec<_> = cr
                .collision_meshes
                .iter()
                .map(|m| rs_mesh_to_bevy_mesh(m))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| (e, None))?;
            Ok((combined, collisions, part_count))
        });

    let preview = match converted {
        Ok((combined_mesh, collision_meshes, part_count)) => {
            Some(CollisionPreviewGenerated {
                preview_id,
                query: query_str,
                combined_mesh,
                collision_meshes,
                part_count,
            })
        }
        Err((e, span)) => {
            outputs.push(CadhrLangOutput {
                preview_id: Some(preview_id),
                message: e,
                is_error: true,
                error_span: span,
            });
            None
        }
    };

    CollisionJobResult { outputs, preview }
}

fn rs_mesh_to_bevy_mesh(rs_mesh: &RsMesh) -> Result<Mesh, String> {
    let vertices = rs_mesh.vertices();
    if vertices.is_empty() {
        return Ok(empty_mesh());
    }
    let stride = rs_mesh.num_props() as usize;
    if stride != 6 {
        return Err(format!(
            "manifold-rs mesh has unexpected num_props={} (expected 6: xyz+normals)",
            stride
        ));
    }

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(vertices.len() / stride);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(vertices.len() / stride);
    for chunk in vertices.chunks_exact(stride) {
        positions.push([chunk[0], chunk[1], chunk[2]]);
        normals.push([chunk[3], chunk[4], chunk[5]]);
    }

    let indices: Vec<u32> = rs_mesh.indices();

    let mut bevy_mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    bevy_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    bevy_mesh.insert_indices(Indices::U32(indices));
    bevy_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);

    Ok(bevy_mesh)
}
