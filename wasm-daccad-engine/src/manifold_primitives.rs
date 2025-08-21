use serde_wasm_bindgen;
use wasm_bindgen::prelude::*;

#[serde_wasm_bindgen]
struct ManifoldMeshData {}

// External JavaScript functions for manifold-3d operations
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "manifoldBridge"], js_name = "createCube")]
    pub fn js_create_cube(width: f64, height: f64, depth: f64) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "manifoldBridge"], js_name = "getMeshData")]
    fn js_get_mesh_data(manifold: &JsValue) -> JsValue;
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
