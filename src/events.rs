use bevy::prelude::*;
use cadhr_lang::bom::BomEntry;
use cadhr_lang::manifold_bridge::{ControlPoint, EvaluatedNode};
use cadhr_lang::parse::{QueryParam, SrcSpan};
use std::collections::HashMap;
use std::path::PathBuf;

// UI -> CadhrLang: request to generate a preview mesh
#[derive(Message, Clone)]
pub struct GeneratePreviewRequest {
    pub preview_id: u64,
    pub database: String,
    pub query: String,
    pub include_paths: Vec<PathBuf>,
    pub control_point_overrides: HashMap<String, f64>,
    pub query_param_overrides: HashMap<String, f64>,
}

// CadhrLang -> UI: mesh has been generated for a request
#[derive(Message)]
pub struct PreviewGenerated {
    pub preview_id: u64,
    pub query: String,
    pub mesh: Mesh,
    pub evaluated_nodes: Vec<EvaluatedNode>,
    pub control_points: Vec<ControlPoint>,
    pub bom_entries: Vec<BomEntry>,
    pub query_params: Vec<QueryParam>,
}

// UI -> CadhrLang: request collision check preview
#[derive(Message, Clone)]
pub struct GenerateCollisionPreviewRequest {
    pub preview_id: u64,
    pub database: String,
    pub query: String,
    pub include_paths: Vec<PathBuf>,
}

// CadhrLang -> UI: collision check result
#[derive(Message)]
pub struct CollisionPreviewGenerated {
    pub preview_id: u64,
    pub query: String,
    pub combined_mesh: Mesh,
    pub collision_meshes: Vec<Mesh>,
    pub part_count: usize,
}

// CadhrLang -> UI: error or log message from cadhr-lang execution
#[derive(Message)]
pub struct CadhrLangOutput {
    pub preview_id: Option<u64>,
    pub message: String,
    pub is_error: bool,
    pub error_span: Option<SrcSpan>,
}
