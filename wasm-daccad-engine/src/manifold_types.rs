// Auto-generated Rust types from manifold-3d TypeScript definitions (simplified)
use serde::{Deserialize, Serialize};

// Basic geometric types
pub type Vec2 = [f64; 2];
pub type Vec3 = [f64; 3];
pub type Mat3 = [f64; 9];
pub type Mat4 = [f64; 16];
pub type SimplePolygon = Vec<Vec2>;
pub type Polygons = Vec<SimplePolygon>;

// Enums for manifold operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FillRule {
    EvenOdd,
    NonZero,
    Positive,
    Negative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JoinType {
    Square,
    Round,
    Miter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorStatus {
    NoError,
    NonFiniteVertex,
    NotManifold,
    VertexOutOfBounds,
    PropertiesWrongLength,
    MissingPositionProperties,
    MergeVectorsDifferentLengths,
    MergeIndexOutOfBounds,
    TransformWrongLength,
    RunIndexWrongLength,
    FaceIDWrongLength,
    InvalidConstruction,
}

// Mesh data structure matching manifold-3d's Mesh interface
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshData {
    /// Number of properties per vertex, always >= 3
    pub num_prop: u32,
    
    /// Flat, GL-style interleaved list of all vertex properties
    /// First three properties are always the position x, y, z
    pub vert_properties: Vec<f32>,
    
    /// The vertex indices of the three triangle corners in CCW order
    pub tri_verts: Vec<u32>,
    
    /// Optional: merge vectors for manifold reconstruction
    pub merge_from_vert: Option<Vec<u32>>,
    pub merge_to_vert: Option<Vec<u32>>,
    
    /// Optional: triangle run information for multi-material meshes
    pub run_index: Option<Vec<u32>>,
    pub run_original_id: Option<Vec<u32>>,
    pub run_transform: Option<Vec<f32>>,
    
    /// Optional: face IDs for maintaining edges during simplification
    pub face_id: Option<Vec<u32>>,
    
    /// Optional: tangent vectors for smooth surfaces
    pub halfedge_tangent: Option<Vec<f32>>,
}

// Bounding box
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min: Vec3,
    pub max: Vec3,
}

// Rectangular bounds for 2D operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

// Smoothness parameter for manifold operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Smoothness {
    pub halfedge: u32,
    pub smoothness: f64,
}

impl MeshData {
    /// Get the number of vertices in the mesh
    pub fn num_vert(&self) -> usize {
        if self.num_prop == 0 {
            0
        } else {
            self.vert_properties.len() / (self.num_prop as usize)
        }
    }
    
    /// Get the number of triangles in the mesh
    pub fn num_tri(&self) -> usize {
        self.tri_verts.len() / 3
    }
    
    /// Get position of a specific vertex
    pub fn get_position(&self, vertex_index: usize) -> Option<Vec3> {
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
    
    /// Get triangle vertex indices
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