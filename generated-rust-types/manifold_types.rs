// Auto-generated Rust types from manifold-3d TypeScript definitions

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fmt::Debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncapsulatedMeshOptions {
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

#[wasm_bindgen]
extern "C" {
    type CrossSection;

    #[wasm_bindgen(constructor)]
    fn new(contours: Polygons, fill_rule: FillRule ) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = square)]
    fn square(size: f64, center: boolean ) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = circle)]
    fn circle(radius: f64, circular_segments: number ) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = union)]
    fn union(a: /* Union: Polygons | CrossSection */ String, b: /* Union: Polygons | CrossSection */ String) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = difference)]
    fn difference(a: /* Union: Polygons | CrossSection */ String, b: /* Union: Polygons | CrossSection */ String) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = intersection)]
    fn intersection(a: /* Union: Polygons | CrossSection */ String, b: /* Union: Polygons | CrossSection */ String) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = union)]
    fn union(polygons: Vec</* Union: Polygons | CrossSection */ String>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = difference)]
    fn difference(polygons: Vec</* Union: Polygons | CrossSection */ String>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = intersection)]
    fn intersection(polygons: Vec</* Union: Polygons | CrossSection */ String>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = hull)]
    fn hull(polygons: Vec</* Union: Polygons | CrossSection */ String>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = compose)]
    fn compose(polygons: Vec</* Union: Polygons | CrossSection */ String>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = of_polygons)]
    fn of_polygons(contours: Polygons, fill_rule: FillRule ) -> CrossSection;

    #[wasm_bindgen(method)]
    fn extrude(this: &CrossSection, height: f64, n_divisions: number , twist_degrees: number , scale_top: f64, center: boolean ) -> Manifold;

    #[wasm_bindgen(method)]
    fn revolve(this: &CrossSection, circular_segments: number , revolve_degrees: number ) -> Manifold;

    #[wasm_bindgen(method)]
    fn transform(this: &CrossSection, m: Mat3) -> CrossSection;

    #[wasm_bindgen(method)]
    fn translate(this: &CrossSection, v: [number, number]) -> CrossSection;

    #[wasm_bindgen(method)]
    fn translate(this: &CrossSection, x: f64, y: number ) -> CrossSection;

    #[wasm_bindgen(method)]
    fn rotate(this: &CrossSection, degrees: f64) -> CrossSection;

    #[wasm_bindgen(method)]
    fn scale(this: &CrossSection, v: f64) -> CrossSection;

    #[wasm_bindgen(method)]
    fn mirror(this: &CrossSection, ax: [number, number]) -> CrossSection;

    #[wasm_bindgen(method)]
    fn warp(this: &CrossSection, warp_func: fn()) -> CrossSection;

    #[wasm_bindgen(method)]
    fn offset(this: &CrossSection, delta: f64, join_type: JoinType , miter_limit: number , circular_segments: number ) -> CrossSection;

    #[wasm_bindgen(method)]
    fn simplify(this: &CrossSection, epsilon: number ) -> CrossSection;

    #[wasm_bindgen(method)]
    fn add(this: &CrossSection, other: /* Union: Polygons | CrossSection */ String) -> CrossSection;

    #[wasm_bindgen(method)]
    fn subtract(this: &CrossSection, other: /* Union: Polygons | CrossSection */ String) -> CrossSection;

    #[wasm_bindgen(method)]
    fn intersect(this: &CrossSection, other: /* Union: Polygons | CrossSection */ String) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = hull)]
    fn hull() -> CrossSection;

    #[wasm_bindgen(method)]
    fn decompose(this: &CrossSection) -> Vec<CrossSection>;

    #[wasm_bindgen(method)]
    fn to_polygons(this: &CrossSection) -> Vec<SimplePolygon>;

    #[wasm_bindgen(method)]
    fn area(this: &CrossSection) -> f64;

    #[wasm_bindgen(method)]
    fn is_empty(this: &CrossSection) -> bool;

    #[wasm_bindgen(method)]
    fn num_vert(this: &CrossSection) -> f64;

    #[wasm_bindgen(method)]
    fn num_contour(this: &CrossSection) -> f64;

    #[wasm_bindgen(method)]
    fn bounds(this: &CrossSection) -> Rect;

    #[wasm_bindgen(method)]
    fn delete(this: &CrossSection) -> ();

}

#[wasm_bindgen]
extern "C" {
    type Manifold;

    #[wasm_bindgen(constructor)]
    fn new(mesh: Mesh) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = tetrahedron)]
    fn tetrahedron() -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = cube)]
    fn cube(size: f64, center: boolean ) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = cylinder)]
    fn cylinder(height: f64, radius_low: f64, radius_high: number , circular_segments: number , center: boolean ) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = sphere)]
    fn sphere(radius: f64, circular_segments: number ) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = extrude)]
    fn extrude(polygons: /* Union: Polygons | CrossSection */ String, height: f64, n_divisions: number , twist_degrees: number , scale_top: f64, center: boolean ) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = revolve)]
    fn revolve(polygons: /* Union: Polygons | CrossSection */ String, circular_segments: number , revolve_degrees: number ) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = of_mesh)]
    fn of_mesh(mesh: Mesh) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = smooth)]
    fn smooth(mesh: Mesh, sharpened_edges: Vec<()>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = level_set)]
    fn level_set(sdf: fn(), bounds: Box, edge_length: f64, level: number , tolerance: number ) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = union)]
    fn union(a: Manifold, b: Manifold) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = difference)]
    fn difference(a: Manifold, b: Manifold) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = intersection)]
    fn intersection(a: Manifold, b: Manifold) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = union)]
    fn union(manifolds: Vec<Manifold>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = difference)]
    fn difference(manifolds: Vec<Manifold>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = intersection)]
    fn intersection(manifolds: Vec<Manifold>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = hull)]
    fn hull(points: Vec</* Union: Manifold | Vec3 */ String>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = compose)]
    fn compose(manifolds: Vec<Manifold>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = reserve_i_ds)]
    fn reserve_i_ds(count: f64) -> f64;

    #[wasm_bindgen(method)]
    fn transform(this: &Manifold, m: Mat4) -> Manifold;

    #[wasm_bindgen(method)]
    fn translate(this: &Manifold, v: [number, number, number]) -> Manifold;

    #[wasm_bindgen(method)]
    fn translate(this: &Manifold, x: f64, y: number , z: number ) -> Manifold;

    #[wasm_bindgen(method)]
    fn rotate(this: &Manifold, v: [number, number, number]) -> Manifold;

    #[wasm_bindgen(method)]
    fn rotate(this: &Manifold, x: f64, y: number , z: number ) -> Manifold;

    #[wasm_bindgen(method)]
    fn scale(this: &Manifold, v: f64) -> Manifold;

    #[wasm_bindgen(method)]
    fn mirror(this: &Manifold, normal: [number, number, number]) -> Manifold;

    #[wasm_bindgen(method)]
    fn warp(this: &Manifold, warp_func: fn()) -> Manifold;

    #[wasm_bindgen(method)]
    fn smooth_by_normals(this: &Manifold, normal_idx: f64) -> Manifold;

    #[wasm_bindgen(method)]
    fn smooth_out(this: &Manifold, min_sharp_angle: number , min_smoothness: number ) -> Manifold;

    #[wasm_bindgen(method)]
    fn refine(this: &Manifold, n: f64) -> Manifold;

    #[wasm_bindgen(method)]
    fn refine_to_length(this: &Manifold, length: f64) -> Manifold;

    #[wasm_bindgen(method)]
    fn refine_to_tolerance(this: &Manifold, tolerance: f64) -> Manifold;

    #[wasm_bindgen(method)]
    fn set_properties(this: &Manifold, num_prop: f64, prop_func: Vec<()>) -> Manifold;

    #[wasm_bindgen(method)]
    fn calculate_curvature(this: &Manifold, gaussian_idx: f64, mean_idx: f64) -> Manifold;

    #[wasm_bindgen(method)]
    fn calculate_normals(this: &Manifold, normal_idx: f64, min_sharp_angle: f64) -> Manifold;

    #[wasm_bindgen(method)]
    fn add(this: &Manifold, other: Manifold) -> Manifold;

    #[wasm_bindgen(method)]
    fn subtract(this: &Manifold, other: Manifold) -> Manifold;

    #[wasm_bindgen(method)]
    fn intersect(this: &Manifold, other: Manifold) -> Manifold;

    #[wasm_bindgen(method)]
    fn split(this: &Manifold, cutter: Manifold) -> [Manifold, Manifold];

    #[wasm_bindgen(method)]
    fn split_by_plane(this: &Manifold, normal: [number, number, number], origin_offset: f64) -> [Manifold, Manifold];

    #[wasm_bindgen(method)]
    fn trim_by_plane(this: &Manifold, normal: [number, number, number], origin_offset: f64) -> Manifold;

    #[wasm_bindgen(method)]
    fn slice(this: &Manifold, height: f64) -> CrossSection;

    #[wasm_bindgen(method)]
    fn project(this: &Manifold) -> CrossSection;

    #[wasm_bindgen(static_method_of = Manifold, js_name = hull)]
    fn hull() -> Manifold;

    #[wasm_bindgen(method)]
    fn decompose(this: &Manifold) -> Vec<Manifold>;

    #[wasm_bindgen(method)]
    fn is_empty(this: &Manifold) -> bool;

    #[wasm_bindgen(method)]
    fn num_vert(this: &Manifold) -> f64;

    #[wasm_bindgen(method)]
    fn num_tri(this: &Manifold) -> f64;

    #[wasm_bindgen(method)]
    fn num_edge(this: &Manifold) -> f64;

    #[wasm_bindgen(method)]
    fn num_prop(this: &Manifold) -> f64;

    #[wasm_bindgen(method)]
    fn num_prop_vert(this: &Manifold) -> f64;

    #[wasm_bindgen(method)]
    fn bounding_box(this: &Manifold) -> Box;

    #[wasm_bindgen(method)]
    fn tolerance(this: &Manifold) -> f64;

    #[wasm_bindgen(method)]
    fn set_tolerance(this: &Manifold, tolerance: f64) -> Manifold;

    #[wasm_bindgen(method)]
    fn simplify(this: &Manifold, tolerance: number ) -> Manifold;

    #[wasm_bindgen(method)]
    fn genus(this: &Manifold) -> f64;

    #[wasm_bindgen(method)]
    fn surface_area(this: &Manifold) -> f64;

    #[wasm_bindgen(method)]
    fn volume(this: &Manifold) -> f64;

    #[wasm_bindgen(method)]
    fn min_gap(this: &Manifold, other: Manifold, search_length: f64) -> f64;

    #[wasm_bindgen(method)]
    fn status(this: &Manifold) -> ErrorStatus;

    #[wasm_bindgen(method)]
    fn get_mesh(this: &Manifold, normal_idx: number ) -> Mesh;

    #[wasm_bindgen(method)]
    fn as_original(this: &Manifold) -> Manifold;

    #[wasm_bindgen(method)]
    fn original_i_d(this: &Manifold) -> f64;

    #[wasm_bindgen(method)]
    fn delete(this: &Manifold) -> ();

}

#[wasm_bindgen]
extern "C" {
    type Mesh;

    #[wasm_bindgen(constructor)]
    fn new(options: MeshOptions) -> Mesh;

    #[wasm_bindgen(method)]
    fn merge(this: &Mesh) -> bool;

    #[wasm_bindgen(method)]
    fn verts(this: &Mesh, tri: f64) -> Vec<u32>;

    #[wasm_bindgen(method)]
    fn position(this: &Mesh, vert: f64) -> Vec<f32>;

    #[wasm_bindgen(method)]
    fn extras(this: &Mesh, vert: f64) -> Vec<f32>;

    #[wasm_bindgen(method)]
    fn tangent(this: &Mesh, halfedge: f64) -> Vec<f32>;

    #[wasm_bindgen(method)]
    fn transform(this: &Mesh, run: f64) -> Mat4;

}

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

pub type GlobalRect = Rect;

pub type GlobalBox = Box;

pub type GlobalSmoothness = Smoothness;

pub type GlobalFillRule = FillRule;

pub type GlobalJoinType = JoinType;

pub type GlobalErrorStatus = ErrorStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifoldManifoldToplevel {
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

// CrossSection - encapsulated type represented as JSValue
pub type CrossSection = wasm_bindgen::JsValue;

// Manifold - encapsulated type represented as JSValue
pub type Manifold = wasm_bindgen::JsValue;

// Mesh - encapsulated type represented as JSValue
pub type Mesh = wasm_bindgen::JsValue;

