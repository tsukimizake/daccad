use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use truck_modeling::{Curve, Mapped, Surface};
use truck_polymesh::{Point3, PolygonMesh};
// OpenCascade imports
use opencascade::mesh::BRepMesh_IncrementalMesh;
use opencascade::shape::{
    TopoDS_Edge, TopoDS_Face, TopoDS_Shape, TopoDS_Shell, TopoDS_Solid, TopoDS_Vertex, TopoDS_Wire,
};
use opencascade_sys::ffi::gp_Pnt as OCPoint3;

use super::gc;
use super::parser::Expr;

pub type ModelId = usize;

// Define model types
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Model {
    // Legacy truck types
    Point3(truck_modeling::Point3),
    Vertex(Arc<truck_modeling::Vertex>),
    Edge(Arc<truck_modeling::Edge>),
    Wire(Arc<truck_modeling::Wire>),
    Face(Arc<truck_modeling::Face>),
    Shell(Arc<truck_modeling::Shell>),
    Solid(Arc<truck_modeling::Solid>),
    Mesh(Arc<truck_polymesh::PolygonMesh>),

    // New OpenCascade types
    OCPoint3(Point3),
    OCVertex(opencascade::shape::TopoDS_Vertex),
    OCEdge(opencascade::shape::TopoDS_Edge),
    OCWire(opencascade::shape::TopoDS_Wire),
    OCFace(opencascade::shape::TopoDS_Face),
    OCShell(opencascade::shape::TopoDS_Shell),
    OCSolid(opencascade::shape::TopoDS_Solid),
    OCShape(opencascade::shape::TopoDS_Shape),
    OCMesh(opencascade::mesh::BRepMesh_IncrementalMesh),
}

// Type-safe wrappers for model elements (Truck)
#[derive(Debug, Clone)]
pub struct VertexModel(pub Arc<truck_modeling::Vertex>);

#[derive(Debug, Clone)]
pub struct EdgeModel(pub Arc<truck_modeling::Edge>);

#[derive(Debug, Clone)]
pub struct WireModel(pub Arc<truck_modeling::Wire>);

#[derive(Debug, Clone)]
pub struct FaceModel(pub Arc<truck_modeling::Face>);

#[derive(Debug, Clone)]
pub struct ShellModel(pub Arc<truck_modeling::Shell>);

#[derive(Debug, Clone)]
pub struct SolidModel(pub Arc<truck_modeling::Solid>);

#[derive(Debug, Clone)]
pub struct MeshModel(pub Arc<truck_polymesh::PolygonMesh>);

// Type-safe wrappers for OpenCascade model elements
#[derive(Debug, Clone)]
pub struct OCVertexModel(pub opencascade::shape::TopoDS_Vertex);

#[derive(Debug, Clone)]
pub struct OCEdgeModel(pub opencascade::shape::TopoDS_Edge);

#[derive(Debug, Clone)]
pub struct OCWireModel(pub opencascade::shape::TopoDS_Wire);

#[derive(Debug, Clone)]
pub struct OCFaceModel(pub opencascade::shape::TopoDS_Face);

#[derive(Debug, Clone)]
pub struct OCShellModel(pub opencascade::shape::TopoDS_Shell);

#[derive(Debug, Clone)]
pub struct OCSolidModel(pub opencascade::shape::TopoDS_Solid);

#[derive(Debug, Clone)]
pub struct OCShapeModel(pub opencascade::shape::TopoDS_Shape);

#[derive(Debug, Clone)]
pub struct OCMeshModel(pub opencascade::mesh::BRepMesh_IncrementalMesh);

// Implement conversions to Model
impl From<VertexModel> for Model {
    fn from(model: VertexModel) -> Self {
        Model::Vertex(model.0)
    }
}

impl From<EdgeModel> for Model {
    fn from(model: EdgeModel) -> Self {
        Model::Edge(model.0)
    }
}

impl From<WireModel> for Model {
    fn from(model: WireModel) -> Self {
        Model::Wire(model.0)
    }
}

impl From<FaceModel> for Model {
    fn from(model: FaceModel) -> Self {
        Model::Face(model.0)
    }
}

impl From<ShellModel> for Model {
    fn from(model: ShellModel) -> Self {
        Model::Shell(model.0)
    }
}

impl From<SolidModel> for Model {
    fn from(model: SolidModel) -> Self {
        Model::Solid(model.0)
    }
}

impl From<MeshModel> for Model {
    fn from(model: MeshModel) -> Self {
        Model::Mesh(model.0)
    }
}

// Keep the original Arc conversions for backward compatibility
impl From<Arc<truck_modeling::Vertex>> for Model {
    fn from(vertex: Arc<truck_modeling::Vertex>) -> Self {
        Model::Vertex(vertex)
    }
}

impl From<Arc<truck_modeling::Edge>> for Model {
    fn from(edge: Arc<truck_modeling::Edge>) -> Self {
        Model::Edge(edge)
    }
}

impl From<Arc<truck_modeling::Wire>> for Model {
    fn from(wire: Arc<truck_modeling::Wire>) -> Self {
        Model::Wire(wire)
    }
}

impl From<Arc<truck_modeling::Face>> for Model {
    fn from(face: Arc<truck_modeling::Face>) -> Self {
        Model::Face(face)
    }
}

impl From<Arc<truck_modeling::Shell>> for Model {
    fn from(shell: Arc<truck_modeling::Shell>) -> Self {
        Model::Shell(shell)
    }
}

impl From<Arc<truck_modeling::Solid>> for Model {
    fn from(solid: Arc<truck_modeling::Solid>) -> Self {
        Model::Solid(solid)
    }
}

impl From<Arc<truck_polymesh::PolygonMesh>> for Model {
    fn from(mesh: Arc<truck_polymesh::PolygonMesh>) -> Self {
        Model::Mesh(mesh)
    }
}

// Implement conversions for OpenCascade wrapper structs
impl From<OCVertexModel> for Model {
    fn from(model: OCVertexModel) -> Self {
        Model::OCVertex(model.0)
    }
}

impl From<OCEdgeModel> for Model {
    fn from(model: OCEdgeModel) -> Self {
        Model::OCEdge(model.0)
    }
}

impl From<OCWireModel> for Model {
    fn from(model: OCWireModel) -> Self {
        Model::OCWire(model.0)
    }
}

impl From<OCFaceModel> for Model {
    fn from(model: OCFaceModel) -> Self {
        Model::OCFace(model.0)
    }
}

impl From<OCShellModel> for Model {
    fn from(model: OCShellModel) -> Self {
        Model::OCShell(model.0)
    }
}

impl From<OCSolidModel> for Model {
    fn from(model: OCSolidModel) -> Self {
        Model::OCSolid(model.0)
    }
}

impl From<OCShapeModel> for Model {
    fn from(model: OCShapeModel) -> Self {
        Model::OCShape(model.0)
    }
}

impl From<OCMeshModel> for Model {
    fn from(model: OCMeshModel) -> Self {
        Model::OCMesh(model.0)
    }
}

// Direct conversions for OpenCascade types
impl From<OCPoint3> for Model {
    fn from(point: OCPoint3) -> Self {
        Model::OCPoint3(point)
    }
}

impl From<TopoDS_Vertex> for Model {
    fn from(vertex: TopoDS_Vertex) -> Self {
        Model::OCVertex(vertex)
    }
}

impl From<TopoDS_Edge> for Model {
    fn from(edge: TopoDS_Edge) -> Self {
        Model::OCEdge(edge)
    }
}

impl From<TopoDS_Wire> for Model {
    fn from(wire: TopoDS_Wire) -> Self {
        Model::OCWire(wire)
    }
}

impl From<TopoDS_Face> for Model {
    fn from(face: TopoDS_Face) -> Self {
        Model::OCFace(face)
    }
}

impl From<TopoDS_Shell> for Model {
    fn from(shell: TopoDS_Shell) -> Self {
        Model::OCShell(shell)
    }
}

impl From<TopoDS_Solid> for Model {
    fn from(solid: TopoDS_Solid) -> Self {
        Model::OCSolid(solid)
    }
}

impl From<TopoDS_Shape> for Model {
    fn from(shape: TopoDS_Shape) -> Self {
        Model::OCShape(shape)
    }
}

impl From<BRepMesh_IncrementalMesh> for Model {
    fn from(mesh: BRepMesh_IncrementalMesh) -> Self {
        Model::OCMesh(mesh)
    }
}

// Methods to safely extract specific model types
impl Model {
    // Truck model accessors
    pub fn as_point3(&self) -> Option<&truck_modeling::Point3> {
        match self {
            Model::Point3(p) => Some(p),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn as_vertex(&self) -> Option<&Arc<truck_modeling::Vertex>> {
        match self {
            Model::Vertex(v) => Some(v),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn as_edge(&self) -> Option<&Arc<truck_modeling::Edge>> {
        match self {
            Model::Edge(e) => Some(e),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn as_wire(&self) -> Option<&Arc<truck_modeling::Wire>> {
        match self {
            Model::Wire(w) => Some(w),
            _ => None,
        }
    }

    pub fn as_face(&self) -> Option<&Arc<truck_modeling::Face>> {
        match self {
            Model::Face(f) => Some(f),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn as_shell(&self) -> Option<&Arc<truck_modeling::Shell>> {
        match self {
            Model::Shell(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_solid(&self) -> Option<&Arc<truck_modeling::Solid>> {
        match self {
            Model::Solid(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_mesh(&self) -> Option<&Arc<truck_polymesh::PolygonMesh>> {
        match self {
            Model::Mesh(m) => Some(m),
            _ => None,
        }
    }

    // OpenCascade model accessors
    pub fn as_oc_point3(&self) -> Option<&OCPoint3> {
        match self {
            Model::OCPoint3(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_oc_vertex(&self) -> Option<&TopoDS_Vertex> {
        match self {
            Model::OCVertex(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_oc_edge(&self) -> Option<&TopoDS_Edge> {
        match self {
            Model::OCEdge(e) => Some(e),
            _ => None,
        }
    }

    pub fn as_oc_wire(&self) -> Option<&TopoDS_Wire> {
        match self {
            Model::OCWire(w) => Some(w),
            _ => None,
        }
    }

    pub fn as_oc_face(&self) -> Option<&TopoDS_Face> {
        match self {
            Model::OCFace(f) => Some(f),
            _ => None,
        }
    }

    pub fn as_oc_shell(&self) -> Option<&TopoDS_Shell> {
        match self {
            Model::OCShell(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_oc_solid(&self) -> Option<&TopoDS_Solid> {
        match self {
            Model::OCSolid(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_oc_shape(&self) -> Option<&TopoDS_Shape> {
        match self {
            Model::OCShape(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_oc_mesh(&self) -> Option<&BRepMesh_IncrementalMesh> {
        match self {
            Model::OCMesh(m) => Some(m),
            _ => None,
        }
    }
}

static COUNTER: AtomicUsize = AtomicUsize::new(1);

pub fn gen_id() -> usize {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug)]
pub struct Env {
    parent: Option<Arc<Mutex<Env>>>,
    vars: HashMap<String, Arc<Expr>>,
    depth: usize,
    models: HashMap<ModelId, Arc<Model>>,
    preview_list: Vec<ModelId>,
}

impl Env {
    pub fn collect_garbage(&mut self) {
        gc::collect_garbage(self);
    }
    pub fn new() -> Env {
        Env {
            parent: None,
            vars: HashMap::new(),
            depth: 0,
            models: HashMap::new(),
            preview_list: Vec::new(),
        }
    }

    pub fn make_child(parent: &Arc<Mutex<Env>>) -> Arc<Mutex<Env>> {
        Arc::new(Mutex::new(Env {
            parent: Some(parent.clone()),
            vars: HashMap::new(),
            depth: parent.lock().unwrap().depth + 1,
            models: HashMap::new(),
            preview_list: Vec::new(),
        }))
    }

    pub fn insert(&mut self, name: String, value: Arc<Expr>) {
        self.vars.insert(name, value);
    }
    pub fn get(&self, name: &str) -> Option<Arc<Expr>> {
        self.vars.get(name).cloned().or_else(|| {
            self.parent
                .as_ref()
                .and_then(|parent| parent.lock().unwrap().get(name))
        })
    }

    pub fn insert_model<T: Into<Model>>(&mut self, model_into: T) -> ModelId {
        let model = model_into.into();
        let id = gen_id();
        self.models.insert(id, Arc::new(model));
        id
    }

    #[allow(dead_code)]
    pub fn get_model(&self, id: ModelId) -> Option<Arc<Model>> {
        self.models.get(&id).cloned().or_else(|| {
            self.parent
                .as_ref()
                .and_then(|parent| parent.lock().unwrap().get_model(id))
        })
    }
    pub fn insert_preview_list(&mut self, id: ModelId) {
        self.preview_list.push(id);
    }

    pub fn polys(&self) -> Vec<(ModelId, Arc<PolygonMesh>)> {
        self.models
            .iter()
            .filter_map(|(id, model)| {
                if let Model::Mesh(mesh) = model.as_ref() {
                    Some((*id, mesh.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn preview_list(&self) -> Vec<ModelId> {
        self.preview_list.clone()
    }

    pub fn vars(&self) -> &HashMap<String, Arc<Expr>> {
        &self.vars
    }

    #[allow(dead_code)]
    pub fn vars_mut(&mut self) -> &mut HashMap<String, Arc<Expr>> {
        &mut self.vars
    }
    pub fn parent(&self) -> &Option<Arc<Mutex<Env>>> {
        &self.parent
    }

    pub fn retain_polys<F>(&mut self, mut f: F)
    where
        F: FnMut(&ModelId, &mut Arc<Model>) -> bool,
    {
        self.models.retain(|k, v| f(k, v));
    }
}

impl PartialEq for Env {
    fn eq(&self, other: &Self) -> bool {
        self.vars == other.vars && self.depth == other.depth
    }
}

// Utility functions for extracting values from expressions
pub mod extract {
    use super::*;
    use crate::lisp::parser::Expr;
    use std::sync::{Arc, Mutex};

    /// Extract a numeric value (f64) from an expression
    pub fn number(expr: &Expr) -> Result<f64, String> {
        match expr {
            Expr::Integer { value, .. } => Ok(*value as f64),
            Expr::Double { value, .. } => Ok(*value),
            _ => Err(format!("Expected number, got {:?}", expr)),
        }
    }

    /// Extract a model from an expression and get a specific type
    pub fn model<F, T>(
        expr: &Expr,
        env: &Arc<Mutex<Env>>,
        extractor: F,
        type_name: &str,
    ) -> Result<T, String>
    where
        F: FnOnce(&Model) -> Option<T>,
    {
        match expr {
            Expr::Model { id, .. } => {
                let model = env
                    .lock()
                    .unwrap()
                    .get_model(*id)
                    .ok_or_else(|| format!("Model with id {} not found", id))?;

                extractor(model.as_ref()).ok_or_else(|| format!("Expected {} model", type_name))
            }
            _ => Err(format!("Expected model, got {:?}", expr)),
        }
    }

    // Truck model extractors

    /// Extract a vertex from an expression
    #[allow(dead_code)]
    pub fn vertex(
        expr: &Expr,
        env: &Arc<Mutex<Env>>,
    ) -> Result<Arc<truck_modeling::Vertex>, String> {
        model(expr, env, |m| m.as_vertex().cloned(), "vertex")
    }

    /// Extract an edge from an expression
    #[allow(dead_code)]
    pub fn edge(expr: &Expr, env: &Arc<Mutex<Env>>) -> Result<Arc<truck_modeling::Edge>, String> {
        model(expr, env, |m| m.as_edge().cloned(), "edge")
    }

    /// Extract a wire from an expression
    #[allow(dead_code)]
    pub fn wire(expr: &Expr, env: &Arc<Mutex<Env>>) -> Result<Arc<truck_modeling::Wire>, String> {
        model(expr, env, |m| m.as_wire().cloned(), "wire")
    }

    /// Extract a face from an expression
    pub fn face(expr: &Expr, env: &Arc<Mutex<Env>>) -> Result<Arc<truck_modeling::Face>, String> {
        model(expr, env, |m| m.as_face().cloned(), "face")
    }

    /// Extract a shell from an expression
    #[allow(dead_code)]
    pub fn shell(expr: &Expr, env: &Arc<Mutex<Env>>) -> Result<Arc<truck_modeling::Shell>, String> {
        model(expr, env, |m| m.as_shell().cloned(), "shell")
    }

    /// Extract a solid from an expression
    pub fn solid(expr: &Expr, env: &Arc<Mutex<Env>>) -> Result<Arc<truck_modeling::Solid>, String> {
        model(expr, env, |m| m.as_solid().cloned(), "solid")
    }

    /// Extract a mesh from an expression
    #[allow(dead_code)]
    pub fn mesh(
        expr: &Expr,
        env: &Arc<Mutex<Env>>,
    ) -> Result<Arc<truck_polymesh::PolygonMesh>, String> {
        model(expr, env, |m| m.as_mesh().cloned(), "mesh")
    }

    /// Extract a point3 from an expression
    pub fn point3(expr: &Expr, env: &Arc<Mutex<Env>>) -> Result<truck_modeling::Point3, String> {
        model(expr, env, |m| m.as_point3().cloned(), "point3")
    }

    // OpenCascade model extractors

    /// Extract an OpenCascade Point3 from an expression
    pub fn oc_point3(expr: &Expr, env: &Arc<Mutex<Env>>) -> Result<OCPoint3, String> {
        model(expr, env, |m| m.as_oc_point3().cloned(), "oc_point3")
    }

    /// Extract an OpenCascade Vertex from an expression
    pub fn oc_vertex(expr: &Expr, env: &Arc<Mutex<Env>>) -> Result<TopoDS_Vertex, String> {
        model(expr, env, |m| m.as_oc_vertex().cloned(), "oc_vertex")
    }

    /// Extract an OpenCascade Edge from an expression
    pub fn oc_edge(expr: &Expr, env: &Arc<Mutex<Env>>) -> Result<TopoDS_Edge, String> {
        model(expr, env, |m| m.as_oc_edge().cloned(), "oc_edge")
    }

    /// Extract an OpenCascade Wire from an expression
    pub fn oc_wire(expr: &Expr, env: &Arc<Mutex<Env>>) -> Result<TopoDS_Wire, String> {
        model(expr, env, |m| m.as_oc_wire().cloned(), "oc_wire")
    }

    /// Extract an OpenCascade Face from an expression
    pub fn oc_face(expr: &Expr, env: &Arc<Mutex<Env>>) -> Result<TopoDS_Face, String> {
        model(expr, env, |m| m.as_oc_face().cloned(), "oc_face")
    }

    /// Extract an OpenCascade Shell from an expression
    pub fn oc_shell(expr: &Expr, env: &Arc<Mutex<Env>>) -> Result<TopoDS_Shell, String> {
        model(expr, env, |m| m.as_oc_shell().cloned(), "oc_shell")
    }

    /// Extract an OpenCascade Solid from an expression
    pub fn oc_solid(expr: &Expr, env: &Arc<Mutex<Env>>) -> Result<TopoDS_Solid, String> {
        model(expr, env, |m| m.as_oc_solid().cloned(), "oc_solid")
    }

    /// Extract an OpenCascade Shape from an expression
    pub fn oc_shape(expr: &Expr, env: &Arc<Mutex<Env>>) -> Result<TopoDS_Shape, String> {
        model(expr, env, |m| m.as_oc_shape().cloned(), "oc_shape")
    }

    /// Extract an OpenCascade Mesh from an expression
    pub fn oc_mesh(expr: &Expr, env: &Arc<Mutex<Env>>) -> Result<BRepMesh_IncrementalMesh, String> {
        model(expr, env, |m| m.as_oc_mesh().cloned(), "oc_mesh")
    }
}

inventory::collect!(LispPrimitive);
inventory::collect!(LispSpecialForm);

#[doc(hidden)]
pub(crate) struct LispPrimitive {
    pub name: &'static str,
    pub func: fn(&[Arc<Expr>], Arc<Mutex<crate::lisp::env::Env>>) -> Result<Arc<Expr>, String>,
}

#[doc(hidden)]
pub(crate) struct LispSpecialForm {
    pub name: &'static str,
    pub func: fn(&[Arc<Expr>], Arc<Mutex<crate::lisp::env::Env>>) -> Result<Arc<Expr>, String>,
}
