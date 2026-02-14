use bevy::prelude::*;

// UI -> CadhrLang: request to generate a preview mesh
#[derive(Message, Clone)]
pub struct GeneratePreviewRequest {
    pub preview_id: u64,
    pub database: String,
    pub query: String,
}

// CadhrLang -> UI: mesh has been generated for a request
#[derive(Message)]
pub struct PreviewGenerated {
    pub preview_id: u64,
    pub query: String,
    pub mesh: Mesh,
}

// CadhrLang -> UI: error or log message from cadhr-lang execution
#[derive(Message)]
pub struct CadhrLangOutput {
    pub message: String,
    pub is_error: bool,
}
