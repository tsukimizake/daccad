use bevy::prelude::*;

// UI -> Prolog: request to generate a preview mesh
#[derive(Message, Clone)]
pub struct GeneratePreviewRequest {
    pub request_id: u64,
    pub query: String,
}

// Prolog -> UI: mesh has been generated for a request
#[derive(Message)]
pub struct PreviewGenerated {
    pub request_id: u64,
    pub query: String,
    pub mesh: Mesh,
}
