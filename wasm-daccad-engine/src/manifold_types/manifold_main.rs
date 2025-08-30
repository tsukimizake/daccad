// Auto-generated Rust types from manifold.d.ts

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use std::fmt::Debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifoldToplevel {
    pub cross_section: CrossSection,
    pub manifold: Manifold,
    pub mesh: Mesh,
    pub triangulate: triangulate,
    pub set_min_circular_angle: setMinCircularAngle,
    pub set_min_circular_edge_length: setMinCircularEdgeLength,
    pub set_circular_segments: setCircularSegments,
    pub get_circular_segments: getCircularSegments,
    pub reset_to_circular_defaults: resetToCircularDefaults,
    pub setup: fn(),
}

pub type CrossSection = wasm_bindgen::JsValue;

pub type Manifold = wasm_bindgen::JsValue;

pub type Mesh = wasm_bindgen::JsValue;

