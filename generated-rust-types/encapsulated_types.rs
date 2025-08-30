// Auto-generated Rust types from manifold-encapsulated-types.d.ts

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use std::fmt::Debug;
use super::todo_unions::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshOptions {
    pub num_prop: f64,
    pub vert_properties: Float32Array,
    pub tri_verts: Uint32Array,
    pub merge_from_vert: Option<Option<Uint32Array>>,
    pub merge_to_vert: Option<Option<Uint32Array>>,
    pub run_index: Option<Option<Uint32Array>>,
    pub run_original_i_d: Option<Option<Uint32Array>>,
    pub run_transform: Option<Option<Float32Array>>,
    pub face_i_d: Option<Option<Uint32Array>>,
    pub halfedge_tangent: Option<Option<Float32Array>>,
}

#[wasm_bindgen]
extern "C" {
    type CrossSection;

    #[wasm_bindgen(constructor)]
    fn new(contours: Todo001Union, fill_rule: Option<FillRule>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = square)]
    fn square(size: Todo005Union, center: Todo006Union) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = circle)]
    fn circle(radius: f64, circular_segments: Option<f64>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = union)]
    fn union(a: Todo007Union, b: Todo007Union) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = difference)]
    fn difference(a: Todo007Union, b: Todo007Union) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = intersection)]
    fn intersection(a: Todo007Union, b: Todo007Union) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = union)]
    fn union(polygons: Vec<Todo007Union>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = difference)]
    fn difference(polygons: Vec<Todo007Union>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = intersection)]
    fn intersection(polygons: Vec<Todo007Union>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = hull)]
    fn hull(polygons: Vec<Todo007Union>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = compose)]
    fn compose(polygons: Vec<Todo007Union>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = of_polygons)]
    fn of_polygons(contours: Todo001Union, fill_rule: Option<FillRule>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn square(this: &CrossSection, size: Todo005Union, center: Todo006Union) -> CrossSection;

    #[wasm_bindgen(method)]
    fn circle(this: &CrossSection, radius: f64, circular_segments: Option<f64>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn extrude(this: &CrossSection, height: f64, n_divisions: Option<f64>, twist_degrees: Option<f64>, scale_top: Todo005Union, center: Todo006Union) -> Manifold;

    #[wasm_bindgen(method)]
    fn revolve(this: &CrossSection, circular_segments: Option<f64>, revolve_degrees: Option<f64>) -> Manifold;

    #[wasm_bindgen(method)]
    fn transform(this: &CrossSection, m: [f64; 9]) -> CrossSection;

    #[wasm_bindgen(method)]
    fn translate(this: &CrossSection, v: [f64; 2]) -> CrossSection;

    #[wasm_bindgen(method)]
    fn translate(this: &CrossSection, x: f64, y: Option<f64>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn rotate(this: &CrossSection, degrees: f64) -> CrossSection;

    #[wasm_bindgen(method)]
    fn scale(this: &CrossSection, v: Todo008Union) -> CrossSection;

    #[wasm_bindgen(method)]
    fn mirror(this: &CrossSection, ax: [f64; 2]) -> CrossSection;

    #[wasm_bindgen(method)]
    fn warp(this: &CrossSection, warp_func: fn()) -> CrossSection;

    #[wasm_bindgen(method)]
    fn offset(this: &CrossSection, delta: f64, join_type: Option<JoinType>, miter_limit: Option<f64>, circular_segments: Option<f64>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn simplify(this: &CrossSection, epsilon: Option<f64>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn add(this: &CrossSection, other: Todo007Union) -> CrossSection;

    #[wasm_bindgen(method)]
    fn subtract(this: &CrossSection, other: Todo007Union) -> CrossSection;

    #[wasm_bindgen(method)]
    fn intersect(this: &CrossSection, other: Todo007Union) -> CrossSection;

    #[wasm_bindgen(method)]
    fn union(this: &CrossSection, a: Todo007Union, b: Todo007Union) -> CrossSection;

    #[wasm_bindgen(method)]
    fn difference(this: &CrossSection, a: Todo007Union, b: Todo007Union) -> CrossSection;

    #[wasm_bindgen(method)]
    fn intersection(this: &CrossSection, a: Todo007Union, b: Todo007Union) -> CrossSection;

    #[wasm_bindgen(method)]
    fn union(this: &CrossSection, polygons: Vec<Todo007Union>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn difference(this: &CrossSection, polygons: Vec<Todo007Union>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn intersection(this: &CrossSection, polygons: Vec<Todo007Union>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn hull(this: &CrossSection) -> CrossSection;

    #[wasm_bindgen(method)]
    fn hull(this: &CrossSection, polygons: Vec<Todo007Union>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn compose(this: &CrossSection, polygons: Vec<Todo007Union>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn decompose(this: &CrossSection) -> Vec<CrossSection>;

    #[wasm_bindgen(method)]
    fn of_polygons(this: &CrossSection, contours: Todo001Union, fill_rule: Option<FillRule>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn to_polygons(this: &CrossSection) -> Vec<Vec<[f64; 2]>>;

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
    fn cube(size: Todo009Union, center: Todo006Union) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = cylinder)]
    fn cylinder(height: f64, radius_low: f64, radius_high: Option<f64>, circular_segments: Option<f64>, center: Todo006Union) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = sphere)]
    fn sphere(radius: f64, circular_segments: Option<f64>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = extrude)]
    fn extrude(polygons: Todo007Union, height: f64, n_divisions: Option<f64>, twist_degrees: Option<f64>, scale_top: Todo005Union, center: Todo006Union) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = revolve)]
    fn revolve(polygons: Todo007Union, circular_segments: Option<f64>, revolve_degrees: Option<f64>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = of_mesh)]
    fn of_mesh(mesh: Mesh) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = smooth)]
    fn smooth(mesh: Mesh, sharpened_edges: Option<Vec<Smoothness>>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = level_set)]
    fn level_set(sdf: fn(), bounds: Box, edge_length: f64, level: Option<f64>, tolerance: Option<f64>) -> Manifold;

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
    fn hull(points: Vec<Todo010Union>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = compose)]
    fn compose(manifolds: Vec<Manifold>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = reserve_i_ds)]
    fn reserve_i_ds(count: f64) -> f64;

    #[wasm_bindgen(method)]
    fn tetrahedron(this: &Manifold) -> Manifold;

    #[wasm_bindgen(method)]
    fn cube(this: &Manifold, size: Todo009Union, center: Todo006Union) -> Manifold;

    #[wasm_bindgen(method)]
    fn cylinder(this: &Manifold, height: f64, radius_low: f64, radius_high: Option<f64>, circular_segments: Option<f64>, center: Todo006Union) -> Manifold;

    #[wasm_bindgen(method)]
    fn sphere(this: &Manifold, radius: f64, circular_segments: Option<f64>) -> Manifold;

    #[wasm_bindgen(method)]
    fn extrude(this: &Manifold, polygons: Todo007Union, height: f64, n_divisions: Option<f64>, twist_degrees: Option<f64>, scale_top: Todo005Union, center: Todo006Union) -> Manifold;

    #[wasm_bindgen(method)]
    fn revolve(this: &Manifold, polygons: Todo007Union, circular_segments: Option<f64>, revolve_degrees: Option<f64>) -> Manifold;

    #[wasm_bindgen(method)]
    fn of_mesh(this: &Manifold, mesh: Mesh) -> Manifold;

    #[wasm_bindgen(method)]
    fn smooth(this: &Manifold, mesh: Mesh, sharpened_edges: Option<Vec<Smoothness>>) -> Manifold;

    #[wasm_bindgen(method)]
    fn level_set(this: &Manifold, sdf: fn(), bounds: Box, edge_length: f64, level: Option<f64>, tolerance: Option<f64>) -> Manifold;

    #[wasm_bindgen(method)]
    fn transform(this: &Manifold, m: [f64; 16]) -> Manifold;

    #[wasm_bindgen(method)]
    fn translate(this: &Manifold, v: [f64; 3]) -> Manifold;

    #[wasm_bindgen(method)]
    fn translate(this: &Manifold, x: f64, y: Option<f64>, z: Option<f64>) -> Manifold;

    #[wasm_bindgen(method)]
    fn rotate(this: &Manifold, v: [f64; 3]) -> Manifold;

    #[wasm_bindgen(method)]
    fn rotate(this: &Manifold, x: f64, y: Option<f64>, z: Option<f64>) -> Manifold;

    #[wasm_bindgen(method)]
    fn scale(this: &Manifold, v: Todo011Union) -> Manifold;

    #[wasm_bindgen(method)]
    fn mirror(this: &Manifold, normal: [f64; 3]) -> Manifold;

    #[wasm_bindgen(method)]
    fn warp(this: &Manifold, warp_func: fn()) -> Manifold;

    #[wasm_bindgen(method)]
    fn smooth_by_normals(this: &Manifold, normal_idx: f64) -> Manifold;

    #[wasm_bindgen(method)]
    fn smooth_out(this: &Manifold, min_sharp_angle: Option<f64>, min_smoothness: Option<f64>) -> Manifold;

    #[wasm_bindgen(method)]
    fn refine(this: &Manifold, n: f64) -> Manifold;

    #[wasm_bindgen(method)]
    fn refine_to_length(this: &Manifold, length: f64) -> Manifold;

    #[wasm_bindgen(method)]
    fn refine_to_tolerance(this: &Manifold, tolerance: f64) -> Manifold;

    #[wasm_bindgen(method)]
    fn set_properties(this: &Manifold, num_prop: f64, prop_func: fn()) -> Manifold;

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
    fn union(this: &Manifold, a: Manifold, b: Manifold) -> Manifold;

    #[wasm_bindgen(method)]
    fn difference(this: &Manifold, a: Manifold, b: Manifold) -> Manifold;

    #[wasm_bindgen(method)]
    fn intersection(this: &Manifold, a: Manifold, b: Manifold) -> Manifold;

    #[wasm_bindgen(method)]
    fn union(this: &Manifold, manifolds: Vec<Manifold>) -> Manifold;

    #[wasm_bindgen(method)]
    fn difference(this: &Manifold, manifolds: Vec<Manifold>) -> Manifold;

    #[wasm_bindgen(method)]
    fn intersection(this: &Manifold, manifolds: Vec<Manifold>) -> Manifold;

    #[wasm_bindgen(method)]
    fn split(this: &Manifold, cutter: Manifold) -> (Manifold, Manifold);

    #[wasm_bindgen(method)]
    fn split_by_plane(this: &Manifold, normal: [f64; 3], origin_offset: f64) -> (Manifold, Manifold);

    #[wasm_bindgen(method)]
    fn trim_by_plane(this: &Manifold, normal: [f64; 3], origin_offset: f64) -> Manifold;

    #[wasm_bindgen(method)]
    fn slice(this: &Manifold, height: f64) -> CrossSection;

    #[wasm_bindgen(method)]
    fn project(this: &Manifold) -> CrossSection;

    #[wasm_bindgen(method)]
    fn hull(this: &Manifold) -> Manifold;

    #[wasm_bindgen(method)]
    fn hull(this: &Manifold, points: Vec<Todo010Union>) -> Manifold;

    #[wasm_bindgen(method)]
    fn compose(this: &Manifold, manifolds: Vec<Manifold>) -> Manifold;

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
    fn simplify(this: &Manifold, tolerance: Option<f64>) -> Manifold;

    #[wasm_bindgen(method)]
    fn genus(this: &Manifold) -> f64;

    #[wasm_bindgen(method)]
    fn surface_area(this: &Manifold) -> f64;

    #[wasm_bindgen(method)]
    fn volume(this: &Manifold) -> f64;

    #[wasm_bindgen(method)]
    fn min_gap(this: &Manifold, other: Manifold, search_length: f64) -> f64;

    #[wasm_bindgen(method)]
    fn status(this: &Manifold) -> Todo004Union;

    #[wasm_bindgen(method)]
    fn get_mesh(this: &Manifold, normal_idx: Option<f64>) -> Mesh;

    #[wasm_bindgen(method)]
    fn as_original(this: &Manifold) -> Manifold;

    #[wasm_bindgen(method)]
    fn original_i_d(this: &Manifold) -> f64;

    #[wasm_bindgen(method)]
    fn reserve_i_ds(this: &Manifold, count: f64) -> f64;

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
    fn verts(this: &Mesh, tri: f64) -> SealedUint32Array<3>;

    #[wasm_bindgen(method)]
    fn position(this: &Mesh, vert: f64) -> SealedFloat32Array<3>;

    #[wasm_bindgen(method)]
    fn extras(this: &Mesh, vert: f64) -> Float32Array;

    #[wasm_bindgen(method)]
    fn tangent(this: &Mesh, halfedge: f64) -> SealedFloat32Array<4>;

    #[wasm_bindgen(method)]
    fn transform(this: &Mesh, run: f64) -> [f64; 16];

}

