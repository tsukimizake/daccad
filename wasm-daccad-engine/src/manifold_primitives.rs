use serde_wasm_bindgen;
use wasm_bindgen::prelude::*;

// Import the generated manifold types and env types
use crate::env::{ManifoldObject, Point3, gen_id};

// Simplified manifold types for demonstration
pub type ManifoldVec3 = [f64; 3];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MeshData {
    pub num_prop: u32,
    pub vert_properties: Vec<f32>,
    pub tri_verts: Vec<u32>,
    pub merge_from_vert: Option<Vec<u32>>,
    pub merge_to_vert: Option<Vec<u32>>,
    pub run_index: Option<Vec<u32>>,
    pub run_original_id: Option<Vec<u32>>,
    pub run_transform: Option<Vec<f32>>,
    pub face_id: Option<Vec<u32>>,
    pub halfedge_tangent: Option<Vec<f32>>,
}

impl MeshData {
    pub fn num_vert(&self) -> usize {
        if self.num_prop == 0 {
            0
        } else {
            self.vert_properties.len() / (self.num_prop as usize)
        }
    }
    
    pub fn num_tri(&self) -> usize {
        self.tri_verts.len() / 3
    }
    
    pub fn get_position(&self, vertex_index: usize) -> Option<ManifoldVec3> {
        let num_prop = self.num_prop as usize;
        if num_prop < 3 {
            return None;
        }
        
        let start_idx = vertex_index * num_prop;
        if start_idx + 2 >= self.vert_properties.len() {
            return None;
        }
        
        Some([
            self.vert_properties[start_idx] as f64,
            self.vert_properties[start_idx + 1] as f64,
            self.vert_properties[start_idx + 2] as f64,
        ])
    }
    
    pub fn get_triangle(&self, triangle_index: usize) -> Option<[u32; 3]> {
        let start_idx = triangle_index * 3;
        if start_idx + 2 >= self.tri_verts.len() {
            return None;
        }
        
        Some([
            self.tri_verts[start_idx],
            self.tri_verts[start_idx + 1],
            self.tri_verts[start_idx + 2],
        ])
    }
}

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

    // Use serde-wasm-bindgen to deserialize the JavaScript object into our generated type
    let mesh_data: MeshData = serde_wasm_bindgen::from_value(mesh_data_js)
        .map_err(|e| format!("Failed to deserialize mesh data: {}", e))?;

    // Validate mesh data using generated type methods
    if mesh_data.num_prop < 3 {
        return Err("Number of properties per vertex must be at least 3".to_string());
    }

    // Convert vertices using the generated type's helper method
    let mut vertices = Vec::new();
    for i in 0..mesh_data.num_vert() {
        if let Some(pos) = mesh_data.get_position(i) {
            // Convert ManifoldVec3 [f64; 3] to Point3
            vertices.push(Point3::new(pos[0] as f32, pos[1] as f32, pos[2] as f32));
        } else {
            return Err(format!("Failed to get position for vertex {}", i));
        }
    }

    // Convert triangles using the generated type's helper method
    let mut faces = Vec::new();
    for i in 0..mesh_data.num_tri() {
        if let Some(triangle) = mesh_data.get_triangle(i) {
            faces.push([triangle[0] as usize, triangle[1] as usize, triangle[2] as usize]);
        } else {
            return Err(format!("Failed to get triangle {}", i));
        }
    }

    Ok(ManifoldObject {
        id: gen_id(),
        vertices,
        faces,
    })
}

// Helper function to create a cube using manifold types
pub fn create_manifold_cube(size: ManifoldVec3) -> Result<MeshData, String> {
    todo!()
}
