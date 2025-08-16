use wasm_bindgen::prelude::*;

use crate::env::{Env, ManifoldObject, Model, Point3, extract, gen_id};
use crate::eval::assert_arg_count;
use crate::parser::Expr;

// External JavaScript functions for manifold-3d operations
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "manifoldBridge"], js_name = "createCube")]
    fn js_create_cube(width: f64, height: f64, depth: f64) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "manifoldBridge"], js_name = "getMeshData")]
    fn js_get_mesh_data(manifold: &JsValue) -> JsValue;
}

// Helper to create Point3 from coordinates
fn return_model<T: Into<Model>>(model_into: T, env: &mut Env) -> Result<Expr, String> {
    let model = model_into.into();
    let id = env.insert_model(model);
    Ok(Expr::model(id))
}

// Helper to parse mesh data from JavaScript and create ManifoldObject
fn parse_mesh_data(mesh_data_js: JsValue) -> Result<ManifoldObject, String> {
    use wasm_bindgen::JsCast;

    // Try to cast to object and extract vertices/faces arrays
    let obj = mesh_data_js
        .dyn_into::<js_sys::Object>()
        .map_err(|_| "Failed to convert mesh data to object")?;

    // Get vertices array
    let vertices_val = js_sys::Reflect::get(&obj, &JsValue::from_str("vertices"))
        .map_err(|_| "Failed to get vertices from mesh data")?;
    let vertices_array = vertices_val
        .dyn_into::<js_sys::Array>()
        .map_err(|_| "Vertices is not an array")?;

    // Get faces array
    let faces_val = js_sys::Reflect::get(&obj, &JsValue::from_str("faces"))
        .map_err(|_| "Failed to get faces from mesh data")?;
    let faces_array = faces_val
        .dyn_into::<js_sys::Array>()
        .map_err(|_| "Faces is not an array")?;

    // Parse vertices (assuming they come as [x, y, z, x, y, z, ...])
    let mut vertices = Vec::new();
    for i in (0..vertices_array.length()).step_by(3) {
        let x = vertices_array
            .get(i)
            .as_f64()
            .ok_or("Invalid vertex x coordinate")?;
        let y = vertices_array
            .get(i + 1)
            .as_f64()
            .ok_or("Invalid vertex y coordinate")?;
        let z = vertices_array
            .get(i + 2)
            .as_f64()
            .ok_or("Invalid vertex z coordinate")?;
        vertices.push(Point3::new(x, y, z));
    }

    // Parse faces (assuming they come as [i1, i2, i3, i1, i2, i3, ...])
    let mut faces = Vec::new();
    for i in (0..faces_array.length()).step_by(3) {
        let i1 = faces_array.get(i).as_f64().ok_or("Invalid face index 1")? as usize;
        let i2 = faces_array
            .get(i + 1)
            .as_f64()
            .ok_or("Invalid face index 2")? as usize;
        let i3 = faces_array
            .get(i + 2)
            .as_f64()
            .ok_or("Invalid face index 3")? as usize;
        faces.push([i1, i2, i3]);
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

    // Call JavaScript function to create cube and get mesh data
    let manifold_js = js_create_cube(width, height, depth);
    let mesh_data = js_get_mesh_data(&manifold_js);

    // Parse the mesh data from JavaScript to create ManifoldObject
    let manifold_object = parse_mesh_data(mesh_data)?;

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
