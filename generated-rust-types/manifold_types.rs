// Auto-generated Rust types from manifold-3d TypeScript definitions

use wasm_bindgen::prelude::*;
use std::collections::HashMap;
use std::fmt::Debug;

#[wasm_bindgen]
extern "C" {
    type CrossSection;

    #[wasm_bindgen(constructor)]
    fn new(contours: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> */ String, fill_rule: /* Union: () | "EvenOdd" | "NonZero" | "Positive" | "Negative" */ String) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = square)]
    fn square(size: /* Union: () | f64 | [f64; 2] */ String, center: /* Union: () | false | true */ String) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = circle)]
    fn circle(radius: f64, circular_segments: Option<f64>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = union)]
    fn union(a: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String, b: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = difference)]
    fn difference(a: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String, b: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = intersection)]
    fn intersection(a: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String, b: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = union)]
    fn union(polygons: Vec</* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = difference)]
    fn difference(polygons: Vec</* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = intersection)]
    fn intersection(polygons: Vec</* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = hull)]
    fn hull(polygons: Vec</* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = compose)]
    fn compose(polygons: Vec</* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String>) -> CrossSection;

    #[wasm_bindgen(static_method_of = CrossSection, js_name = of_polygons)]
    fn of_polygons(contours: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> */ String, fill_rule: /* Union: () | "EvenOdd" | "NonZero" | "Positive" | "Negative" */ String) -> CrossSection;

    #[wasm_bindgen(method)]
    fn square(this: &CrossSection, size: /* Union: () | f64 | [f64; 2] */ String, center: /* Union: () | false | true */ String) -> CrossSection;

    #[wasm_bindgen(method)]
    fn circle(this: &CrossSection, radius: f64, circular_segments: Option<f64>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn extrude(this: &CrossSection, height: f64, n_divisions: Option<f64>, twist_degrees: Option<f64>, scale_top: /* Union: () | f64 | [f64; 2] */ String, center: /* Union: () | false | true */ String) -> Manifold;

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
    fn scale(this: &CrossSection, v: /* Union: f64 | [f64; 2] */ String) -> CrossSection;

    #[wasm_bindgen(method)]
    fn mirror(this: &CrossSection, ax: [f64; 2]) -> CrossSection;

    #[wasm_bindgen(method)]
    fn warp(this: &CrossSection, warp_func: (vert: Vec2) => void) -> CrossSection;

    #[wasm_bindgen(method)]
    fn offset(this: &CrossSection, delta: f64, join_type: /* Union: () | "Square" | "Round" | "Miter" */ String, miter_limit: Option<f64>, circular_segments: Option<f64>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn simplify(this: &CrossSection, epsilon: Option<f64>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn add(this: &CrossSection, other: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String) -> CrossSection;

    #[wasm_bindgen(method)]
    fn subtract(this: &CrossSection, other: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String) -> CrossSection;

    #[wasm_bindgen(method)]
    fn intersect(this: &CrossSection, other: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String) -> CrossSection;

    #[wasm_bindgen(method)]
    fn union(this: &CrossSection, a: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String, b: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String) -> CrossSection;

    #[wasm_bindgen(method)]
    fn difference(this: &CrossSection, a: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String, b: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String) -> CrossSection;

    #[wasm_bindgen(method)]
    fn intersection(this: &CrossSection, a: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String, b: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String) -> CrossSection;

    #[wasm_bindgen(method)]
    fn union(this: &CrossSection, polygons: Vec</* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn difference(this: &CrossSection, polygons: Vec</* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn intersection(this: &CrossSection, polygons: Vec</* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn hull(this: &CrossSection) -> CrossSection;

    #[wasm_bindgen(method)]
    fn hull(this: &CrossSection, polygons: Vec</* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn compose(this: &CrossSection, polygons: Vec</* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String>) -> CrossSection;

    #[wasm_bindgen(method)]
    fn decompose(this: &CrossSection) -> Vec<CrossSection>;

    #[wasm_bindgen(method)]
    fn of_polygons(this: &CrossSection, contours: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> */ String, fill_rule: /* Union: () | "EvenOdd" | "NonZero" | "Positive" | "Negative" */ String) -> CrossSection;

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
    fn cube(size: /* Union: () | f64 | [f64; 3] */ String, center: /* Union: () | false | true */ String) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = cylinder)]
    fn cylinder(height: f64, radius_low: f64, radius_high: Option<f64>, circular_segments: Option<f64>, center: /* Union: () | false | true */ String) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = sphere)]
    fn sphere(radius: f64, circular_segments: Option<f64>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = extrude)]
    fn extrude(polygons: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String, height: f64, n_divisions: Option<f64>, twist_degrees: Option<f64>, scale_top: /* Union: () | f64 | [f64; 2] */ String, center: /* Union: () | false | true */ String) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = revolve)]
    fn revolve(polygons: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String, circular_segments: Option<f64>, revolve_degrees: Option<f64>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = of_mesh)]
    fn of_mesh(mesh: Mesh) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = smooth)]
    fn smooth(mesh: Mesh, sharpened_edges: Option<Vec<Smoothness>>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = level_set)]
    fn level_set(sdf: (point: Vec3) => number, bounds: Box, edge_length: f64, level: Option<f64>, tolerance: Option<f64>) -> Manifold;

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
    fn hull(points: Vec</* Union: Manifold | [f64; 3] */ String>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = compose)]
    fn compose(manifolds: Vec<Manifold>) -> Manifold;

    #[wasm_bindgen(static_method_of = Manifold, js_name = reserve_i_ds)]
    fn reserve_i_ds(count: f64) -> f64;

    #[wasm_bindgen(method)]
    fn tetrahedron(this: &Manifold) -> Manifold;

    #[wasm_bindgen(method)]
    fn cube(this: &Manifold, size: /* Union: () | f64 | [f64; 3] */ String, center: /* Union: () | false | true */ String) -> Manifold;

    #[wasm_bindgen(method)]
    fn cylinder(this: &Manifold, height: f64, radius_low: f64, radius_high: Option<f64>, circular_segments: Option<f64>, center: /* Union: () | false | true */ String) -> Manifold;

    #[wasm_bindgen(method)]
    fn sphere(this: &Manifold, radius: f64, circular_segments: Option<f64>) -> Manifold;

    #[wasm_bindgen(method)]
    fn extrude(this: &Manifold, polygons: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String, height: f64, n_divisions: Option<f64>, twist_degrees: Option<f64>, scale_top: /* Union: () | f64 | [f64; 2] */ String, center: /* Union: () | false | true */ String) -> Manifold;

    #[wasm_bindgen(method)]
    fn revolve(this: &Manifold, polygons: /* Union: Vec<[f64; 2]> | Vec<Vec<[f64; 2]>> | CrossSection */ String, circular_segments: Option<f64>, revolve_degrees: Option<f64>) -> Manifold;

    #[wasm_bindgen(method)]
    fn of_mesh(this: &Manifold, mesh: Mesh) -> Manifold;

    #[wasm_bindgen(method)]
    fn smooth(this: &Manifold, mesh: Mesh, sharpened_edges: Option<Vec<Smoothness>>) -> Manifold;

    #[wasm_bindgen(method)]
    fn level_set(this: &Manifold, sdf: (point: Vec3) => number, bounds: Box, edge_length: f64, level: Option<f64>, tolerance: Option<f64>) -> Manifold;

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
    fn scale(this: &Manifold, v: /* Union: f64 | [f64; 3] */ String) -> Manifold;

    #[wasm_bindgen(method)]
    fn mirror(this: &Manifold, normal: [f64; 3]) -> Manifold;

    #[wasm_bindgen(method)]
    fn warp(this: &Manifold, warp_func: (vert: Vec3) => void) -> Manifold;

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
    fn set_properties(this: &Manifold, num_prop: f64, prop_func: (newProp: number[], position: Vec3, oldProp: number[]) => void) -> Manifold;

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
    fn hull(this: &Manifold, points: Vec</* Union: Manifold | [f64; 3] */ String>) -> Manifold;

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
    fn status(this: &Manifold) -> /* Union: "NoError" | "NonFiniteVertex" | "NotManifold" | "VertexOutOfBounds" | "PropertiesWrongLength" | "MissingPositionProperties" | "MergeVectorsDifferentLengths" | "MergeIndexOutOfBounds" | "TransformWrongLength" | "RunIndexWrongLength" | "FaceIDWrongLength" | "InvalidConstruction" */ String;

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

