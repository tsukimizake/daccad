use wasm_bindgen::prelude::*;

use crate::env::{Env, ManifoldObject, Model, Point3, extract, gen_id};
use crate::eval::assert_arg_count;
use crate::parser::Expr;

// External JavaScript functions for manifold-3d operations
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "manifoldBridge"], js_name = "createCube")]
    fn js_create_cube(width: f64, height: f64, depth: f64) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "manifoldBridge"], js_name = "createCylinder")]
    fn js_create_cylinder(height: f64, radius: f64, segments: Option<i32>) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "manifoldBridge"], js_name = "getMeshData")]
    fn js_get_mesh_data(manifold: &JsValue) -> JsValue;
}

// Helper to create Point3 from coordinates
fn return_model<T: Into<Model>>(model_into: T, env: &mut Env) -> Result<Expr, String> {
    let model = model_into.into();
    let id = env.insert_model(model);
    Ok(Expr::model(id))
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
    let _mesh_data = js_get_mesh_data(&manifold_js);

    // For now, create a simple manifold object with placeholder data
    // In a full implementation, we would parse the mesh_data from JavaScript
    let manifold_object = ManifoldObject {
        id: gen_id(),
        vertices: vec![
            Point3::new(-width / 2.0, -height / 2.0, -depth / 2.0),
            Point3::new(width / 2.0, -height / 2.0, -depth / 2.0),
            Point3::new(width / 2.0, height / 2.0, -depth / 2.0),
            Point3::new(-width / 2.0, height / 2.0, -depth / 2.0),
            Point3::new(-width / 2.0, -height / 2.0, depth / 2.0),
            Point3::new(width / 2.0, -height / 2.0, depth / 2.0),
            Point3::new(width / 2.0, height / 2.0, depth / 2.0),
            Point3::new(-width / 2.0, height / 2.0, depth / 2.0),
        ],
        faces: vec![
            [0, 1, 2],
            [2, 3, 0], // bottom
            [4, 7, 6],
            [6, 5, 4], // top
            [0, 4, 5],
            [5, 1, 0], // front
            [2, 6, 7],
            [7, 3, 2], // back
            [0, 3, 7],
            [7, 4, 0], // left
            [1, 5, 6],
            [6, 2, 1], // right
        ],
    };

    return_model(manifold_object, env)
}

/// Create a cylinder with given height and radius
/// (cylinder height radius [segments])
pub fn cylinder(args: &[Expr], env: &mut Env) -> Result<Expr, String> {
    assert_arg_count(args, 2..=3)?;

    let height = extract::number(&args[0])?;
    let radius = extract::number(&args[1])?;
    let segments = if args.len() > 2 {
        Some(extract::number(&args[2])? as i32)
    } else {
        None
    };

    // Call JavaScript function to create cylinder
    let _manifold_js = js_create_cylinder(height, radius, segments);

    // Create a simple cylinder approximation (8-sided) for now
    let manifold_object = ManifoldObject {
        id: gen_id(),
        vertices: vec![
            // Bottom circle
            Point3::new(radius, 0.0, -height / 2.0),
            Point3::new(radius * 0.707, radius * 0.707, -height / 2.0),
            Point3::new(0.0, radius, -height / 2.0),
            Point3::new(-radius * 0.707, radius * 0.707, -height / 2.0),
            Point3::new(-radius, 0.0, -height / 2.0),
            Point3::new(-radius * 0.707, -radius * 0.707, -height / 2.0),
            Point3::new(0.0, -radius, -height / 2.0),
            Point3::new(radius * 0.707, -radius * 0.707, -height / 2.0),
            // Top circle
            Point3::new(radius, 0.0, height / 2.0),
            Point3::new(radius * 0.707, radius * 0.707, height / 2.0),
            Point3::new(0.0, radius, height / 2.0),
            Point3::new(-radius * 0.707, radius * 0.707, height / 2.0),
            Point3::new(-radius, 0.0, height / 2.0),
            Point3::new(-radius * 0.707, -radius * 0.707, height / 2.0),
            Point3::new(0.0, -radius, height / 2.0),
            Point3::new(radius * 0.707, -radius * 0.707, height / 2.0),
        ],
        faces: vec![
            // Side faces
            [0, 1, 9],
            [9, 8, 0],
            [1, 2, 10],
            [10, 9, 1],
            [2, 3, 11],
            [11, 10, 2],
            [3, 4, 12],
            [12, 11, 3],
            [4, 5, 13],
            [13, 12, 4],
            [5, 6, 14],
            [14, 13, 5],
            [6, 7, 15],
            [15, 14, 6],
            [7, 0, 8],
            [8, 15, 7],
        ],
    };

    return_model(manifold_object, env)
}

/// Boolean union of two manifolds
/// (union manifold1 manifold2)
pub fn union(args: &[Expr], _env: &mut Env) -> Result<Expr, String> {
    assert_arg_count(args, 2)?;

    // This is a placeholder - in a real implementation, we'd need to:
    // 1. Extract the manifold objects from the expressions
    // 2. Perform the union operation using manifold-3d
    // 3. Return the result as a new manifold

    // For now, return the first argument as a placeholder
    Ok(args[0].clone())
}

/// Boolean subtraction of two manifolds  
/// (subtract manifold1 manifold2)
pub fn subtract(args: &[Expr], _env: &mut Env) -> Result<Expr, String> {
    assert_arg_count(args, 2)?;

    // Placeholder implementation
    Ok(args[0].clone())
}

/// Boolean intersection of two manifolds
/// (intersect manifold1 manifold2)  
pub fn intersect(args: &[Expr], _env: &mut Env) -> Result<Expr, String> {
    assert_arg_count(args, 2)?;

    // Placeholder implementation
    Ok(args[0].clone())
}

/// Translate a manifold by a vector
/// (translate manifold dx dy dz)
pub fn translate(args: &[Expr], _env: &mut Env) -> Result<Expr, String> {
    assert_arg_count(args, 4)?;

    // Placeholder implementation
    Ok(args[0].clone())
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
