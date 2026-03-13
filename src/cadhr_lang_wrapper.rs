use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::tasks::AsyncComputeTaskPool;
use bevy::{mesh::Indices, prelude::*};
use bevy_async_ecs::AsyncWorld;

use crate::events::{
    CadhrLangOutput, CollisionPreviewGenerated, GenerateCollisionPreviewRequest,
    GeneratePreviewRequest, PreviewGenerated,
};
use cadhr_lang::bom::BomExtractor;
use cadhr_lang::manifold_bridge::{MeshGenerator, extract_control_points};
use cadhr_lang::module::resolve_modules;
use cadhr_lang::parse::{
    SrcSpan, collect_query_params, database, parse_error_span,
    query as parse_query, substitute_query_params,
};
use cadhr_lang::term_processor::TermProcessor;
use cadhr_lang::term_rewrite::{CadhrError, execute, infer_query_param_ranges};
use manifold_rs::Mesh as RsMesh;

pub struct CadhrLangPlugin;

impl Plugin for CadhrLangPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (start_request_worker, start_collision_worker));
    }
}

fn start_request_worker(world: &mut World) {
    let async_world = AsyncWorld::from_world(world);
    AsyncComputeTaskPool::get()
        .spawn(async move {
            let requests = async_world
                .start_waiting_for_messages::<GeneratePreviewRequest>()
                .await;
            loop {
                let req = requests.wait().await;
                spawn_mesh_job(async_world.clone(), req);
            }
        })
        .detach();
}

fn spawn_mesh_job(async_world: AsyncWorld, req: GeneratePreviewRequest) {
    AsyncComputeTaskPool::get()
        .spawn(async move {
            let preview_id = req.preview_id;
            let db_src = req.database;
            let query = req.query;

            let mut logs: Vec<String> = Vec::new();

            // Parse, discover query params, substitute overrides, execute
            let resolve_result = (|| -> Result<(Vec<cadhr_lang::parse::ScopedTerm>, Vec<cadhr_lang::manifold_bridge::ControlPoint>, Vec<cadhr_lang::parse::QueryParam>), (String, Option<SrcSpan>)> {
                let (_, query_terms) =
                    parse_query(&query).map_err(|e| {
                        let span = parse_error_span(&query, &e);
                        (format!("Query parse error: {:?}", e), span)
                    })?;
                let db =
                    database(&db_src).map_err(|e| {
                        let span = parse_error_span(&db_src, &e);
                        (format!("Database parse error: {:?}", e), span)
                    })?;
                let mut db = resolve_modules(
                    db,
                    &req.include_paths,
                    &mut std::collections::HashSet::new(),
                )
                .map_err(|e| (format!("Module error: {}", e), None))?;

                let mut query_params = collect_query_params(&query_terms);
                infer_query_param_ranges(&query_terms, &db, &mut query_params)
                    .map_err(|e| (format!("Range inference error: {}", e), None))?;

                // Build substitution values: override > default_value > midpoint > 0
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
                        let span = e.span();
                        (format!("Rewrite error: {}", e), span)
                    })?;
                logs.push(format!("Resolved terms: {:?}", resolved));

                let control_points = extract_control_points(&mut resolved, &req.control_point_overrides);
                env.update_query_param_ranges(&mut query_params);
                Ok((resolved, control_points, query_params))
            })();

            let log_message = logs.join("\n");
            if !log_message.is_empty() {
                async_world
                    .send_message(CadhrLangOutput {
                        preview_id: None,
                        message: log_message,
                        is_error: false,
                        error_span: None,
                    })
                    .await;
            }

            let (resolved, control_points, query_params) = match resolve_result {
                Ok(triple) => triple,
                Err((e, span)) => {
                    bevy::log::error!("Failed to resolve: {}", e);
                    async_world
                        .send_message(CadhrLangOutput {
                            preview_id: Some(preview_id),
                            message: e,
                            is_error: true,
                            error_span: span,
                        })
                        .await;
                    let empty_mesh = Mesh::new(
                        PrimitiveTopology::TriangleList,
                        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
                    );
                    async_world
                        .send_message(PreviewGenerated {
                            preview_id,
                            query,
                            mesh: empty_mesh,
                            evaluated_nodes: vec![],
                            control_points: vec![],
                            bom_entries: vec![],
                            query_params: vec![],
                        })
                        .await;
                    return;
                }
            };

            let bom_entries = BomExtractor
                .process(&resolved)
                .unwrap_or_else(|e| {
                    bevy::log::warn!("BOM extraction warning: {}", e);
                    vec![]
                });

            if resolved.is_empty() {
                let empty_mesh = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
                );
                async_world
                    .send_message(PreviewGenerated {
                        preview_id,
                        query,
                        mesh: empty_mesh,
                        evaluated_nodes: vec![],
                        control_points,
                        bom_entries,
                        query_params,
                    })
                    .await;
                return;
            }

            let mesh_generator = MeshGenerator {
                include_paths: req.include_paths.clone(),
            };
            let mesh_result = mesh_generator
                .process(&resolved)
                .map_err(|e| {
                    let span = e.span();
                    (format!("Mesh error: {}", e), span)
                })
                .and_then(|(rs_mesh, evaluated_nodes)| {
                    rs_mesh_to_bevy_mesh(&rs_mesh)
                        .map(|m| (m, evaluated_nodes))
                        .map_err(|e| (e, None))
                });

            match mesh_result {
                Ok((mesh, evaluated_nodes)) => {
                    async_world
                        .send_message(PreviewGenerated {
                            preview_id,
                            query,
                            mesh,
                            evaluated_nodes,
                            control_points,
                            bom_entries,
                            query_params,
                        })
                        .await;
                }
                Err((e, span)) => {
                    bevy::log::error!("Failed to generate mesh: {}", e);
                    async_world
                        .send_message(CadhrLangOutput {
                            preview_id: Some(preview_id),
                            message: e,
                            is_error: true,
                            error_span: span,
                        })
                        .await;
                    let empty_mesh = Mesh::new(
                        PrimitiveTopology::TriangleList,
                        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
                    );
                    async_world
                        .send_message(PreviewGenerated {
                            preview_id,
                            query,
                            mesh: empty_mesh,
                            evaluated_nodes: vec![],
                            control_points,
                            bom_entries,
                            query_params,
                        })
                        .await;
                }
            }
        })
        .detach();
}

fn start_collision_worker(world: &mut World) {
    let async_world = AsyncWorld::from_world(world);
    AsyncComputeTaskPool::get()
        .spawn(async move {
            let requests = async_world
                .start_waiting_for_messages::<GenerateCollisionPreviewRequest>()
                .await;
            loop {
                let req = requests.wait().await;
                spawn_collision_job(async_world.clone(), req);
            }
        })
        .detach();
}

fn spawn_collision_job(async_world: AsyncWorld, req: GenerateCollisionPreviewRequest) {
    AsyncComputeTaskPool::get()
        .spawn(async move {
            let preview_id = req.preview_id;
            let db_src = req.database;
            let query_str = req.query;

            let result = (|| -> Result<
                (Vec<cadhr_lang::manifold_bridge::ManifoldExpr>, Vec<std::path::PathBuf>),
                (String, Option<cadhr_lang::parse::SrcSpan>),
            > {
                let (_, query_terms) = parse_query(&query_str).map_err(|e| {
                    let span = parse_error_span(&query_str, &e);
                    (format!("Query parse error: {:?}", e), span)
                })?;
                let db = database(&db_src).map_err(|e| {
                    let span = parse_error_span(&db_src, &e);
                    (format!("Database parse error: {:?}", e), span)
                })?;
                let mut db = resolve_modules(
                    db,
                    &req.include_paths,
                    &mut std::collections::HashSet::new(),
                )
                .map_err(|e| (format!("Module error: {}", e), None))?;
                let (resolved, _) = execute(&mut db, query_terms).map_err(|e| {
                    let span = e.span();
                    (format!("Rewrite error: {}", e), span)
                })?;

                use cadhr_lang::manifold_bridge::{ConversionError, ManifoldExpr};
                let exprs: Vec<ManifoldExpr> = resolved
                    .iter()
                    .filter_map(|t| match ManifoldExpr::from_term(t) {
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
                    async_world
                        .send_message(CadhrLangOutput {
                            preview_id: Some(preview_id),
                            message: e,
                            is_error: true,
                            error_span: span,
                        })
                        .await;
                    return;
                }
            };

            use cadhr_lang::collision::check_collisions;
            // Convert manifold-rs results to bevy meshes before any .await
            // (manifold_rs::Mesh is !Send)
            let converted = check_collisions(&exprs, &include_paths)
                .map_err(|e| {
                    let span = e.span();
                    (format!("Collision check error: {}", e), span)
                })
                .and_then(|cr| {
                    let part_count = cr.part_count;
                    let combined = rs_mesh_to_bevy_mesh(&cr.combined_mesh)
                        .map_err(|e| (e, None))?;
                    let collisions: Vec<_> = cr
                        .collision_meshes
                        .iter()
                        .map(|m| rs_mesh_to_bevy_mesh(m))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| (e, None))?;
                    Ok((combined, collisions, part_count))
                });

            match converted {
                Ok((combined_mesh, collision_meshes, part_count)) => {
                    async_world
                        .send_message(CollisionPreviewGenerated {
                            preview_id,
                            query: query_str,
                            combined_mesh,
                            collision_meshes,
                            part_count,
                        })
                        .await;
                }
                Err((e, span)) => {
                    async_world
                        .send_message(CadhrLangOutput {
                            preview_id: Some(preview_id),
                            message: e,
                            is_error: true,
                            error_span: span,
                        })
                        .await;
                }
            }
        })
        .detach();
}

// Convert a manifold-rs Mesh into a Bevy Mesh
fn rs_mesh_to_bevy_mesh(rs_mesh: &RsMesh) -> Result<Mesh, String> {
    let vertices = rs_mesh.vertices();
    if vertices.is_empty() {
        return Ok(Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        ));
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

    // Indices are already triangles from manifold-rs
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
