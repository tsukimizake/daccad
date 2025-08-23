// Auto-generated Rust types from manifold-3d TypeScript definitions

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fmt::Debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshOptions {
    pub num_prop: f64,
    pub vert_properties: Vec<f32>,
    pub tri_verts: Vec<u32>,
    pub merge_from_vert: Option<Uint32Array >,
    pub merge_to_vert: Option<Uint32Array >,
    pub run_index: Option<Uint32Array >,
    pub run_original_i_d: Option<Uint32Array >,
    pub run_transform: Option<Float32Array >,
    pub face_i_d: Option<Uint32Array >,
    pub halfedge_tangent: Option<Float32Array >,
}

// CrossSection from manifold-encapsulated-types - represented as JSValue
pub type CrossSection = wasm_bindgen::JsValue;

// Manifold from manifold-encapsulated-types - represented as JSValue
pub type Manifold = wasm_bindgen::JsValue;

// Mesh from manifold-encapsulated-types - represented as JSValue
pub type Mesh = wasm_bindgen::JsValue;

// Fixed-size array type for SealedUint32Array
pub type SealedUint32Array<const N: usize> = [u32; N];

// Fixed-size array type for SealedFloat32Array
pub type SealedFloat32Array<const N: usize> = [f32; N];

pub type Vec2 = [f64; 2];

pub type Vec3 = [f64; 3];

pub type Mat3 = [f64; 9];

pub type Mat4 = [f64; 16];

pub type SimplePolygon = Vec<Vec2>;

pub type Polygons = Vec<SimplePolygon>;

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

