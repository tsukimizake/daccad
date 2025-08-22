// Auto-generated Rust types from manifold-3d TypeScript definitions

use std::collections::HashMap;
use std::fmt::Debug;

#[derive(Debug, Clone)]
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

pub type CrossSection = CrossSection;

pub type Manifold = Manifold;

pub type Mesh = Mesh;

#[derive(Debug, Clone)]
pub struct SealedUint32Array {
    pub length: N,
}

#[derive(Debug, Clone)]
pub struct SealedFloat32Array {
    pub length: N,
}

pub type Vec2 = [f64; 2];

pub type Vec3 = [f64; 3];

pub type Mat3 = [f64; 9];

pub type Mat4 = [f64; 16];

pub type SimplePolygon = Vec<Vec2>;

pub type Polygons = Vec<SimplePolygon>;

pub type Rect = Rect;

pub type Box = Box;

pub type Smoothness = Smoothness;

pub type FillRule = FillRule;

pub type JoinType = JoinType;

pub type ErrorStatus = ErrorStatus;

#[derive(Debug, Clone)]
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

