use elm_rs::{Elm, ElmDecode, ElmEncode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tsify::Tsify;

#[derive(
    Debug, Clone, Serialize, Deserialize, Elm, ElmDecode, ElmEncode, Tsify, PartialEq, Eq, Hash,
)]
pub struct ModelId(pub usize);

impl From<usize> for ModelId {
    fn from(id: usize) -> Self {
        ModelId(id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Point3 {
            x: x as f64,
            y: y as f64,
            z: z as f64,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifoldObject {
    // TODO
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshData {
    pub vertices: Vec<Point3>,
    pub faces: Vec<[usize; 3]>,
}

#[derive(Debug, Clone)]
pub enum Model {
    Manifold(ManifoldObject),
    Mesh(MeshData),
}

#[derive(Debug, Clone)]
pub struct Env {
    models: HashMap<ModelId, Model>,
    next_id: usize,
}

impl Env {
    pub fn new() -> Self {
        Env {
            models: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn get_model(&self, id: ModelId) -> Option<&Model> {
        self.models.get(&id)
    }

    pub fn add_model(&mut self, model: Model) -> ModelId {
        let id = ModelId(self.next_id);
        self.next_id += 1;
        self.models.insert(id.clone(), model);
        id
    }
}

pub fn gen_id() -> ModelId {
    static mut COUNTER: usize = 0;
    unsafe {
        COUNTER += 1;
        ModelId(COUNTER)
    }
}
