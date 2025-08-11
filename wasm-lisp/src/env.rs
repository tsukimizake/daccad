use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tsify::Tsify;
use elm_rs::{Elm, ElmDecode, ElmEncode};

use super::parser::Expr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Tsify, Elm, ElmDecode, ElmEncode)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ModelId(pub usize);

impl From<usize> for ModelId {
    fn from(id: usize) -> Self {
        ModelId(id)
    }
}

impl From<ModelId> for usize {
    fn from(id: ModelId) -> Self {
        id.0
    }
}

impl ModelId {
    pub fn new(id: usize) -> Self {
        ModelId(id)
    }
    
    pub fn as_usize(&self) -> usize {
        self.0
    }
}

impl std::fmt::Display for ModelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Simplified model types for manifold-3d integration
#[derive(Debug, Clone)]
pub enum Model {
    Point3(Point3),
    Manifold(ManifoldObject), // For manifold-3d meshes
    Mesh(MeshData),           // For raw mesh data
}

#[derive(Debug, Clone)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

// Wrapper for manifold-3d JavaScript objects
#[derive(Debug, Clone)]
pub struct ManifoldObject {
    // This will hold the JavaScript Manifold object reference
    // For now, we'll use a placeholder ID until we implement JS bindings
    pub id: usize,
    pub vertices: Vec<Point3>,
    pub faces: Vec<[usize; 3]>, // Triangle faces
}

#[derive(Debug, Clone)]
pub struct MeshData {
    pub vertices: Vec<Point3>,
    pub faces: Vec<[usize; 3]>, // Triangle faces
}

impl Model {
    pub fn as_point3(&self) -> Option<&Point3> {
        match self {
            Model::Point3(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_manifold(&self) -> Option<&ManifoldObject> {
        match self {
            Model::Manifold(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_mesh(&self) -> Option<&MeshData> {
        match self {
            Model::Mesh(m) => Some(m),
            _ => None,
        }
    }
}

static mut COUNTER: usize = 1;

pub fn gen_id() -> usize {
    unsafe {
        let id = COUNTER;
        COUNTER += 1;
        id
    }
}

#[derive(Debug, Clone)]
pub struct Env {
    parent: Option<Box<Env>>,
    vars: HashMap<String, Expr>,
    depth: usize,
    models: HashMap<ModelId, Model>,
    preview_list: Vec<ModelId>,
}

impl Env {
    pub fn new() -> Env {
        Env {
            parent: None,
            vars: HashMap::new(),
            depth: 0,
            models: HashMap::new(),
            preview_list: Vec::new(),
        }
    }

    pub fn make_child(parent: Env) -> Env {
        let depth = parent.depth + 1;
        Env {
            parent: Some(Box::new(parent)),
            vars: HashMap::new(),
            depth,
            models: HashMap::new(),
            preview_list: Vec::new(),
        }
    }

    pub fn insert(&mut self, name: String, value: Expr) {
        self.vars.insert(name, value);
    }

    pub fn get(&self, name: &str) -> Option<Expr> {
        self.vars
            .get(name)
            .cloned()
            .or_else(|| self.parent.as_ref().and_then(|parent| parent.get(name)))
    }

    pub fn insert_model<T: Into<Model>>(&mut self, model_into: T) -> ModelId {
        let model = model_into.into();
        let id = gen_id();
        let model_id = ModelId::from(id);
        self.models.insert(model_id, model);
        model_id
    }

    pub fn get_model(&self, id: ModelId) -> Option<&Model> {
        self.models
            .get(&id)
            .or_else(|| self.parent.as_ref().and_then(|parent| parent.get_model(id)))
    }

    pub fn insert_preview_list(&mut self, id: ModelId) {
        self.preview_list.push(id);
    }

    pub fn preview_list(&self) -> Vec<ModelId> {
        self.preview_list.clone()
    }

    pub fn vars(&self) -> &HashMap<String, Expr> {
        &self.vars
    }

    pub fn vars_mut(&mut self) -> &mut HashMap<String, Expr> {
        &mut self.vars
    }
}

impl PartialEq for Env {
    fn eq(&self, other: &Self) -> bool {
        self.vars == other.vars && self.depth == other.depth
    }
}

// Conversion implementations
impl From<Point3> for Model {
    fn from(point: Point3) -> Self {
        Model::Point3(point)
    }
}

impl From<ManifoldObject> for Model {
    fn from(manifold: ManifoldObject) -> Self {
        Model::Manifold(manifold)
    }
}

impl From<MeshData> for Model {
    fn from(mesh: MeshData) -> Self {
        Model::Mesh(mesh)
    }
}

// Utility functions for extracting values from expressions
pub mod extract {
    use super::*;

    /// Extract a numeric value (f64) from an expression
    pub fn number(expr: &Expr) -> Result<f64, String> {
        match expr {
            Expr::Integer { value, .. } => Ok(*value as f64),
            Expr::Double { value, .. } => Ok(*value),
            _ => Err(format!("Expected number, got {:?}", expr)),
        }
    }

    /// Extract a model from an expression and get a specific type
    pub fn model<F, T>(expr: &Expr, env: &Env, extractor: F, type_name: &str) -> Result<T, String>
    where
        F: FnOnce(&Model) -> Option<T>,
    {
        match expr {
            Expr::Model { id, .. } => {
                let model = env
                    .get_model(*id)
                    .ok_or_else(|| format!("Model with id {} not found", id))?;

                extractor(model).ok_or_else(|| format!("Expected {} model", type_name))
            }
            _ => Err(format!("Expected model, got {:?}", expr)),
        }
    }

    /// Extract a point3 from an expression
    pub fn point3(expr: &Expr, env: &Env) -> Result<Point3, String> {
        model(expr, env, |m| m.as_point3().cloned(), "point3")
    }

    /// Extract a manifold from an expression
    pub fn manifold(expr: &Expr, env: &Env) -> Result<ManifoldObject, String> {
        model(expr, env, |m| m.as_manifold().cloned(), "manifold")
    }

    /// Extract a mesh from an expression  
    pub fn mesh(expr: &Expr, env: &Env) -> Result<MeshData, String> {
        model(expr, env, |m| m.as_mesh().cloned(), "mesh")
    }
}

// Struct to hold primitives for WASM environment
#[derive(Debug)]
pub struct LispPrimitive {
    pub name: &'static str,
    pub func: fn(&[Expr], &mut Env) -> Result<Expr, String>,
}

#[derive(Debug)]
pub struct LispSpecialForm {
    pub name: &'static str,
    pub func: fn(&[Expr], &mut Env) -> Result<Expr, String>,
}

