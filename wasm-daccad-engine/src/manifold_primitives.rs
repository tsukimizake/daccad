use serde_wasm_bindgen;
use wasm_bindgen::prelude::*;

use crate::env::{Env, ManifoldMeshData, ManifoldObject, Model, Point3, extract, gen_id};
use crate::eval::assert_arg_count;
use crate::parser::Expr;

// External JavaScript functions for manifold-3d operations
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "manifoldBridge"], js_name = "createCube")]
    pub fn js_create_cube(width: f64, height: f64, depth: f64) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "manifoldBridge"], js_name = "getMeshData")]
    fn js_get_mesh_data(manifold: &JsValue) -> JsValue;
}

// Helper to create Point3 from coordinates
fn return_model<T: Into<Model>>(model_into: T, env: &mut Env) -> Result<Expr, String> {
    let model = model_into.into();
    let id = env.insert_model(model);
    Ok(Expr::model(id))
}

// Helper to get mesh data from manifold and parse it into ManifoldObject
pub fn get_and_parse_mesh_data(manifold: &JsValue) -> Result<ManifoldObject, String> {
    // Get mesh data from JavaScript
    let mesh_data_js = js_get_mesh_data(manifold);

    // Use serde-wasm-bindgen to deserialize the JavaScript object
    let mesh_data: ManifoldMeshData = serde_wasm_bindgen::from_value(mesh_data_js)
        .map_err(|e| format!("Failed to deserialize mesh data: {}", e))?;

    // Convert interleaved vertex properties to Point3 structures
    let mut vertices = Vec::new();
    let num_prop = mesh_data.num_prop as usize;
    if num_prop < 3 {
        return Err("Number of properties per vertex must be at least 3".to_string());
    }
    if mesh_data.vert_properties.len() % num_prop != 0 {
        return Err("Vertex properties array length must be divisible by numProp".to_string());
    }

    for chunk in mesh_data.vert_properties.chunks(num_prop) {
        // First 3 properties are always x, y, z position
        vertices.push(Point3::new(chunk[0], chunk[1], chunk[2]));
    }

    // Convert flat triangle vertex indices array to triangle faces
    let mut faces = Vec::new();
    if mesh_data.tri_verts.len() % 3 != 0 {
        return Err("Triangle vertex indices array length must be divisible by 3".to_string());
    }

    for chunk in mesh_data.tri_verts.chunks(3) {
        faces.push([chunk[0] as usize, chunk[1] as usize, chunk[2] as usize]);
    }

    Ok(ManifoldObject {
        id: gen_id(),
        vertices,
        faces,
    })
}

/// Create a point at the specified coordinates
/// (point x y z) or (p x y z)
pub fn point(args: &[Expr], env: &mut Env) -> Result<Expr, String> {
    assert_arg_count(args, 2..=3)?;

    let mut coords = args
        .iter()
        .map(|expr| extract::number(expr))
        .collect::<Result<Vec<_>, String>>()?;

    if coords.len() == 2 {
        coords.push(0.0);
    }

    let point = Point3::new(coords[0], coords[1], coords[2]);
    return_model(point, env)
}

/// Create a cube with given dimensions
/// (cube width height depth)
pub fn cube(args: &[Expr], env: &mut Env) -> Result<Expr, String> {
    assert_arg_count(args, 3)?;

    let width = extract::number(&args[0])?;
    let height = extract::number(&args[1])?;
    let depth = extract::number(&args[2])?;

    // Create cube and get mesh data in one step
    let manifold_js = js_create_cube(width, height, depth);
    let manifold_object = get_and_parse_mesh_data(&manifold_js)?;

    return_model(manifold_object, env)
}

/// Preview a model for rendering
/// (preview manifold)
pub fn preview(args: &[Expr], env: &mut Env) -> Result<Expr, String> {
    assert_arg_count(args, 1)?;

    match &args[0] {
        Expr::Model { id, .. } => {
            env.insert_preview_list(*id);
            Ok(args[0].clone())
        }
        _ => Err("preview: expected model".to_string()),
    }
}
