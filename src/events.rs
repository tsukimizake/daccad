use bevy::prelude::*;

// UI -> CadhrLang: request to generate a preview mesh
#[derive(Message, Clone)]
pub struct GeneratePreviewRequest {
    pub request_id: u64,
    pub database: String,
    pub query: String,
    pub preview_index: Option<usize>, // Some(i) = update existing preview, None = new preview
}

// CadhrLang -> UI: mesh has been generated for a request
#[derive(Message)]
pub struct PreviewGenerated {
    pub request_id: u64,
    pub query: String,
    pub mesh: Mesh,
    pub preview_index: Option<usize>, // Some(i) = update existing preview, None = new preview
}

// CadhrLang -> UI: error or log message from cadhr-lang execution
#[derive(Message)]
pub struct CadhrLangOutput {
    pub message: String,
    pub is_error: bool,
}