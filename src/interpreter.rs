use cadhr_lang::bom::{BomEntry, BomExtractor};
use cadhr_lang::collision::check_collisions;
use cadhr_lang::manifold_bridge::{
    ConversionError, ControlPoint, EvaluatedNode, MeshGenerator, Model3D, extract_control_points,
};
use cadhr_lang::term_rewrite::CadhrError;
use cadhr_lang::module::resolve_modules;
use cadhr_lang::parse::{
    FileRegistry, QueryParam, SrcSpan, collect_query_params, database,
    parse_error_span, query as parse_query, substitute_query_params,
};
use cadhr_lang::term_processor::TermProcessor;
use cadhr_lang::term_rewrite::{execute, infer_query_param_ranges};

use crate::preview::pipeline::Vertex;
use manifold_rs::Mesh as RsMesh;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct MeshJobParams {
    pub database: String,
    pub query: String,
    pub include_paths: Vec<PathBuf>,
    pub control_point_overrides: HashMap<String, f64>,
    pub query_param_overrides: HashMap<String, f64>,
}

#[derive(Clone)]
pub struct MeshJobResult {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub evaluated_nodes: Vec<EvaluatedNode>,
    pub control_points: Vec<ControlPoint>,
    pub bom_entries: Vec<BomEntry>,
    pub query_params: Vec<QueryParam>,
    pub logs: Vec<String>,
    pub error: Option<(String, Option<SrcSpan>)>,
}

impl std::fmt::Debug for MeshJobResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshJobResult")
            .field("vertices_count", &self.vertices.len())
            .field("indices_count", &self.indices.len())
            .field("error", &self.error)
            .finish()
    }
}

fn format_error(
    label: &str,
    msg: &str,
    span: Option<SrcSpan>,
    registry: &FileRegistry,
) -> (String, Option<SrcSpan>) {
    let location = span.map(|s| registry.format_span(&s)).unwrap_or_default();
    let formatted = if location.is_empty() {
        format!("{}: {}", label, msg)
    } else {
        format!("{} at {}: {}", label, location, msg)
    };
    (formatted, span)
}

fn rs_mesh_to_vertices_colored(
    rs_mesh: &RsMesh,
    color: [f32; 4],
) -> Result<(Vec<Vertex>, Vec<u32>), String> {
    let raw = rs_mesh.vertices();
    if raw.is_empty() {
        return Ok((vec![], vec![]));
    }
    let stride = rs_mesh.num_props() as usize;
    if stride != 6 {
        return Err(format!(
            "manifold-rs mesh has unexpected num_props={} (expected 6)",
            stride
        ));
    }
    let vertices: Vec<Vertex> = raw
        .chunks_exact(stride)
        .map(|c| Vertex {
            position: [c[0], c[1], c[2]],
            normal: [c[3], c[4], c[5]],
            color,
        })
        .collect();
    let indices = rs_mesh.indices();
    Ok((vertices, indices))
}

fn append_mesh(
    all_verts: &mut Vec<Vertex>,
    all_idx: &mut Vec<u32>,
    verts: Vec<Vertex>,
    idx: Vec<u32>,
) {
    let base = all_verts.len() as u32;
    all_verts.extend(verts);
    all_idx.extend(idx.iter().map(|&i| i + base));
}

fn rs_mesh_to_vertices(rs_mesh: &RsMesh) -> Result<(Vec<Vertex>, Vec<u32>), String> {
    let raw = rs_mesh.vertices();
    if raw.is_empty() {
        return Ok((vec![], vec![]));
    }
    let stride = rs_mesh.num_props() as usize;
    if stride != 6 {
        return Err(format!(
            "manifold-rs mesh has unexpected num_props={} (expected 6: xyz+normals)",
            stride
        ));
    }
    let vertices: Vec<Vertex> = raw
        .chunks_exact(stride)
        .map(|c| Vertex {
            position: [c[0], c[1], c[2]],
            normal: [c[3], c[4], c[5]],
            color: [0.0, 0.0, 0.0, 0.0],
        })
        .collect();
    let indices = rs_mesh.indices();
    Ok((vertices, indices))
}

pub fn run_mesh_job(params: MeshJobParams) -> MeshJobResult {
    let mut logs = Vec::new();

    let resolve_result = (|| -> Result<
        (Vec<cadhr_lang::parse::ScopedTerm>, Vec<ControlPoint>, Vec<QueryParam>),
        (String, Option<SrcSpan>),
    > {
        let mut file_registry = FileRegistry::new();
        file_registry.register_main("db".to_string(), params.database.clone());
        let query_file_id = file_registry.register("query".to_string(), params.query.clone());

        let (_, query_terms) = parse_query(&params.query).map_err(|e| {
            let span = parse_error_span(&params.query, &e).map(|mut s| {
                s.file_id = query_file_id;
                s
            });
            format_error("Parse error", &format!("{:?}", e), span, &file_registry)
        })?;
        let db = database(&params.database).map_err(|e| {
            let span = parse_error_span(&params.database, &e);
            format_error("Parse error", &format!("{:?}", e), span, &file_registry)
        })?;
        let mut db = resolve_modules(
            db,
            &params.include_paths,
            &mut std::collections::HashSet::new(),
            &mut file_registry,
        )
        .map_err(|e| (format!("Module error: {}", e), None))?;

        let mut query_params = collect_query_params(&query_terms);
        infer_query_param_ranges(&query_terms, &db, &mut query_params)
            .map_err(|e| (format!("Range inference error: {}", e), None))?;

        let mut values = params.query_param_overrides.clone();
        for param in &query_params {
            if !values.contains_key(&param.name) {
                let default = if let Some(dv) = param.default_value {
                    dv.to_f64()
                } else {
                    match (param.min.as_ref(), param.max.as_ref()) {
                        (Some(min), Some(max)) => (min.value.to_f64() + max.value.to_f64()) / 2.0,
                        _ => 0.0,
                    }
                };
                values.insert(param.name.clone(), default);
            }
        }

        let substituted = substitute_query_params(&query_terms, &values);
        logs.push(format!("Query terms: {:?}", substituted));
        logs.push(format!("Database clauses: {:#?}", db));
        let (mut resolved, _env) = execute(&mut db, substituted).map_err(|e| {
            format_error("Rewrite error", &e.to_string(), e.span(), &file_registry)
        })?;
        logs.push(format!("Resolved terms: {:?}", resolved));

        let control_points =
            extract_control_points(&mut resolved, &params.control_point_overrides);
        Ok((resolved, control_points, query_params))
    })();

    let (vertices, indices, evaluated_nodes, control_points, bom_entries, query_params, error) =
        match resolve_result {
            Ok((resolved, control_points, query_params)) => {
                let bom_entries = BomExtractor.process(&resolved).unwrap_or_else(|e| {
                    eprintln!("BOM extraction warning: {}", e);
                    vec![]
                });

                if resolved.is_empty() {
                    return MeshJobResult {
                        vertices: vec![],
                        indices: vec![],
                        evaluated_nodes: vec![],
                        control_points,
                        bom_entries,
                        query_params,
                        logs,
                        error: None,
                    };
                }

                let mesh_generator = MeshGenerator {
                    include_paths: params.include_paths.clone(),
                };
                match mesh_generator.process(&resolved) {
                    Ok((rs_mesh, evaluated_nodes)) => match rs_mesh_to_vertices(&rs_mesh) {
                        Ok((verts, idxs)) => (
                            verts,
                            idxs,
                            evaluated_nodes,
                            control_points,
                            bom_entries,
                            query_params,
                            None,
                        ),
                        Err(e) => (vec![], vec![], vec![], control_points, bom_entries, query_params, Some((e, None))),
                    },
                    Err(e) => {
                        let span = e.span();
                        (
                            vec![],
                            vec![],
                            vec![],
                            control_points,
                            bom_entries,
                            query_params,
                            Some((format!("Mesh error: {}", e), span)),
                        )
                    }
                }
            }
            Err(e) => (vec![], vec![], vec![], vec![], vec![], vec![], Some(e)),
        };

    MeshJobResult {
        vertices,
        indices,
        evaluated_nodes,
        control_points,
        bom_entries,
        query_params,
        logs,
        error,
    }
}

pub struct CollisionJobParams {
    pub database: String,
    pub query: String,
    pub include_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct CollisionJobResult {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub part_count: usize,
    pub collision_count: usize,
    pub error: Option<(String, Option<SrcSpan>)>,
}

pub fn run_collision_job(params: CollisionJobParams) -> CollisionJobResult {
    let error_result = |e| CollisionJobResult {
        vertices: vec![],
        indices: vec![],
        part_count: 0,
        collision_count: 0,
        error: Some(e),
    };

    let resolve_result = (|| -> Result<Vec<cadhr_lang::parse::ScopedTerm>, (String, Option<SrcSpan>)> {
        let mut file_registry = FileRegistry::new();
        file_registry.register_main("db".to_string(), params.database.clone());
        let query_file_id = file_registry.register("query".to_string(), params.query.clone());

        let (_, query_terms) = parse_query(&params.query).map_err(|e| {
            let span = parse_error_span(&params.query, &e).map(|mut s| {
                s.file_id = query_file_id;
                s
            });
            format_error("Parse error", &format!("{:?}", e), span, &file_registry)
        })?;
        let db = database(&params.database).map_err(|e| {
            let span = parse_error_span(&params.database, &e);
            format_error("Parse error", &format!("{:?}", e), span, &file_registry)
        })?;
        let mut db = resolve_modules(
            db,
            &params.include_paths,
            &mut std::collections::HashSet::new(),
            &mut file_registry,
        )
        .map_err(|e| (format!("Module error: {}", e), None))?;

        let (resolved, _env) = execute(&mut db, query_terms).map_err(|e| {
            format_error("Rewrite error", &e.to_string(), e.span(), &file_registry)
        })?;
        Ok(resolved)
    })();

    let resolved = match resolve_result {
        Ok(r) => r,
        Err(e) => return error_result(e),
    };

    let exprs: Result<Vec<Model3D>, _> = resolved
        .iter()
        .filter_map(|t| match Model3D::from_term(t) {
            Ok(e) => Some(Ok(e)),
            Err(ConversionError::UnknownPrimitive(_)) => None,
            Err(e) => Some(Err(e)),
        })
        .collect();

    let exprs = match exprs {
        Ok(e) => e,
        Err(e) => return error_result((format!("Conversion error: {}", e), None)),
    };

    if exprs.is_empty() {
        return error_result(("No mesh terms found in query result".to_string(), None));
    }

    match check_collisions(&exprs, &params.include_paths) {
        Ok(result) => {
            let mut all_verts = Vec::new();
            let mut all_idx = Vec::new();

            // 各パーツを alpha=0 (uniform color) で描画
            match rs_mesh_to_vertices_colored(&result.combined_mesh, [0.0, 0.0, 0.0, 0.0]) {
                Ok((v, i)) => append_mesh(&mut all_verts, &mut all_idx, v, i),
                Err(e) => return error_result((e, None)),
            }

            // 衝突領域を赤 (alpha=1) で描画
            let collision_count = result.collision_meshes.len();
            for mesh in &result.collision_meshes {
                match rs_mesh_to_vertices_colored(mesh, [1.0, 0.15, 0.0, 1.0]) {
                    Ok((v, i)) => append_mesh(&mut all_verts, &mut all_idx, v, i),
                    Err(e) => return error_result((e, None)),
                }
            }

            CollisionJobResult {
                vertices: all_verts,
                indices: all_idx,
                part_count: result.part_count,
                collision_count,
                error: None,
            }
        }
        Err(e) => error_result((format!("Collision error: {}", e), None)),
    }
}
