use bevy::prelude::*;

// UI -> Prolog: request to generate a preview mesh
#[derive(Message, Clone)]
pub struct GeneratePreviewRequest {
    pub request_id: u64,
    pub database: String,
    pub query: String,
    pub preview_index: Option<usize>, // Some(i) = update existing preview, None = new preview
}

// Prolog -> UI: mesh has been generated for a request
#[derive(Message)]
pub struct PreviewGenerated {
    pub request_id: u64,
    pub query: String,
    pub mesh: Mesh,
    pub preview_index: Option<usize>, // Some(i) = update existing preview, None = new preview
}