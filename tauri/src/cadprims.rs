use crate::lisp::env::Env;
use crate::lisp::env::LispPrimitive;
use crate::lisp::env::Model;
use crate::lisp::env::extract;
use crate::lisp::eval::assert_arg_count;
use crate::lisp::eval::eval;
use crate::lisp::parser::Expr;
use inventory;
use lisp_macro::lisp_fn;
use std::sync::{Arc, Mutex};

// Note: OpenCascade integration is currently on hold as the FFI types don't implement Send + Sync
// which makes them incompatible with Tauri's state management.
// We'll need to implement proper wrappers before we can fully integrate OpenCascade.

// Legacy truck imports only for now

// Legacy truck imports - will be phased out gradually
use truck_meshalgo::prelude::*;
use truck_modeling::Solid;
use truck_modeling::builder::{rotated, translated};
use truck_modeling::{Point3, builder};
use truck_topology::EdgeDisplayFormat;
use truck_topology::VertexDisplayFormat;
use truck_topology::WireDisplayFormat;

fn return_model<T: Into<Model>>(model_into: T, env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    let model = model_into.into();
    let id = env.lock().unwrap().insert_model(model);
    Ok(Arc::new(Expr::model(id)))
}

/// Load an STL file into the environment
///
/// # Lisp Usage
/// `(load-stl "path/to/file.stl")`
///
/// # Returns
/// A model expression representing the loaded STL file.
#[lisp_fn]
fn load_stl(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 1)?;
    match args[0].as_ref() {
        Expr::String { value: path, .. } => {
            let reader = std::fs::File::open(path).map_err(|e| e.to_string())?;

            if let Ok(mesh) =
                truck_polymesh::stl::read(&reader, truck_polymesh::stl::StlType::Automatic)
            {
                return_model(Arc::new(mesh), env)
            } else {
                Err("load_stl: failed to read file".to_string())
            }
        }
        _ => Err("load_stl: expected string".to_string()),
    }
}

/// Mark a model for preview/rendering in the UI
///
/// # Lisp Usage
/// `(preview model)`
///
/// # Returns
/// The model that was marked for preview
#[lisp_fn]
fn preview(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    println!("preview: {:?}", args);
    assert_arg_count(args, 1)?;
    match args[0].as_ref() {
        Expr::Model { id, .. } => {
            // Get the model and verify it's a mesh or can be converted to one
            let mut env_guard = env.lock().unwrap();
            let model = env_guard
                .get_model(*id)
                .ok_or_else(|| format!("Model with id {} not found", id))?;

            // Handle different model types
            if model.as_mesh().is_some() {
                // Legacy Truck mesh - add directly to preview list
                env_guard.insert_preview_list(*id);
                Ok(args[0].clone())
            } else if let Some(solid) = model.as_solid() {
                // Legacy Truck solid - convert to mesh and add to preview
                let mesh = Arc::new(solid.triangulation(0.01).to_polygon());
                let id = env_guard.insert_model(Model::Mesh(mesh.clone()));
                env_guard.insert_preview_list(id);

                drop(env_guard);
                return_model(Model::Mesh(mesh), env)
            } else if let Some(shape) = model.as_oc_shape() {
                // OpenCascade shape - convert to mesh for preview
                // Create a mesh from the shape with a deflection of 0.01
                let mut mesh = BRepMesh_IncrementalMesh::new(shape.clone(), 0.01);
                mesh.perform();

                // Store the mesh and add to preview list
                let mesh_id = env_guard.insert_model(Model::OCMesh(mesh));
                env_guard.insert_preview_list(mesh_id);

                drop(env_guard);
                return_model(Model::OCShape(shape.clone()), env)
            } else if let Some(solid) = model.as_oc_solid() {
                // OpenCascade solid - treat as a shape for preview
                let solid_shape = solid.clone();

                // Create a mesh from the solid with a deflection of 0.01
                let mut mesh = BRepMesh_IncrementalMesh::new(solid_shape.clone(), 0.01);
                mesh.perform();

                // Store the mesh and add to preview list
                let mesh_id = env_guard.insert_model(Model::OCMesh(mesh));
                env_guard.insert_preview_list(mesh_id);

                drop(env_guard);
                return_model(Model::OCSolid(solid.clone()), env)
            } else {
                Err("preview: expected solid, mesh, or shape model".to_string())
            }
        }

        _ => Err("preview: expected solid, mesh, or shape model".to_string()),
    }
}

/// Create a point at the specified coordinates
///
/// # Lisp Usage
/// `(point x y z)` or `(point x y)` or `(p x y z)` or `(p x y)`
/// When z is omitted, it defaults to 0.
///
/// # Examples
/// `(p 1 2 3)` - point at (1,2,3)
/// `(p 10 5)` - point at (10,5,0)
///
/// # Returns
/// A point model
#[lisp_fn("p")]
fn point(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 2..=3).map_err(|e| format!("point: {}", e))?;

    let mut coords = args
        .iter()
        .map(|expr| extract::number(expr.as_ref()))
        .collect::<Result<Vec<_>, String>>()?;

    if coords.len() == 2 {
        coords.push(0.0);
    }

    let point = truck_modeling::Point3::new(coords[0], coords[1], coords[2]);
    return_model(Model::Point3(point), env)
}

/// Create a line between two points
///
/// # Lisp Usage
/// `(line point1 point2)`
///
/// # Examples
/// `(line (p 0 0) (p 1 1))` - line from origin to (1,1,0)
///
/// # Returns
/// A line (edge) model
#[lisp_fn]
fn line(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 2)?;

    let points = args
        .iter()
        .map(|expr| extract::point3(expr.as_ref(), &env))
        .collect::<Result<Vec<_>, String>>()?;

    let v1 = Arc::new(truck_modeling::Vertex::new(points[0]));
    let v2 = Arc::new(truck_modeling::Vertex::new(points[1]));
    let edge = truck_modeling::builder::line(&v1, &v2);
    return_model(Model::Edge(Arc::new(edge)), env)
}

/// Create a face from a sequence of points using turtle-like movement
///
/// # Lisp Usage
/// `(turtle point1 point2 point3 ...)`
///
/// # Examples
/// `(turtle (p 0 0) (p 1 0) (p 1 1) (p 0 1))` - square face
///
/// # Returns
/// A face model
#[lisp_fn("turtle")]
fn turtle_sketch(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 3..).map_err(|e| format!("turtle: {}", e))?;

    let points = args
        .iter()
        .map(|expr| extract::point3(expr.as_ref(), &env))
        .collect::<Result<Vec<_>, String>>()?;

    let mut edges = Vec::new();

    // Create vertices from points
    let mut vertices: Vec<Arc<truck_modeling::Vertex>> = Vec::new();
    let mut current_point = points[0];

    let first_vertex = Arc::new(truck_modeling::Vertex::new(current_point));
    vertices.push(first_vertex.clone());

    for i in 1..points.len() {
        let movement = points[i];
        let next_point = truck_modeling::Point3::new(
            current_point.x + movement.x,
            current_point.y + movement.y,
            current_point.z + movement.z,
        );

        let next_vertex = Arc::new(truck_modeling::Vertex::new(next_point));
        vertices.push(next_vertex);
        current_point = next_point;
    }

    // Create edges between vertices
    for i in 0..vertices.len() - 1 {
        let edge = truck_modeling::builder::line(&vertices[i], &vertices[i + 1]);
        edges.push(edge);
    }

    // Create closing edge
    let closing_edge = truck_modeling::builder::line(&vertices.last().unwrap(), &vertices[0]);
    edges.push(closing_edge);

    let wire = truck_modeling::Wire::from_iter(edges.into_iter());

    let face =
        truck_modeling::builder::try_attach_plane(&[wire]).map_err(|e| format!("{:?}", e))?;

    return_model(Arc::new(face), env)
}

/// Create a circle in the XY plane
///
/// # Lisp Usage
/// `(circle x y radius)`
///
/// # Examples
/// `(circle 0 0 5)` - circle at origin with radius 5
///
/// # Returns
/// A circle face model
#[lisp_fn]
fn circle(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 3)?;

    let x = extract::number(args[0].as_ref())?;
    let y = extract::number(args[1].as_ref())?;
    let radius = extract::number(args[2].as_ref())?;

    let v1 = builder::vertex(Point3::new(x - radius, y, 0.0));
    let v2 = builder::vertex(Point3::new(x, y, 0.0));
    let p3 = Point3::new(x + radius, y, 0.0);

    let edge = truck_modeling::builder::circle_arc(&v1, &v2, p3);

    let wire = truck_modeling::Wire::from(vec![edge]);
    let face =
        truck_modeling::builder::try_attach_plane(&[wire]).map_err(|e| format!("{:?}", e))?;

    return_model(Model::Face(Arc::new(face)), env)
}

/// Extrude a face along the Z axis
///
/// # Lisp Usage
/// `(linear-extrude face height)`
///
/// # Examples
/// `(linear-extrude (circle 0 0 5) 10)` - cylinder with radius 5 and height 10
///
/// # Returns
/// A solid model
#[lisp_fn]
fn linear_extrude(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 2)?;

    let face = extract::face(args[0].as_ref(), &env)?;
    let height = extract::number(args[1].as_ref())?;
    let solid = truck_modeling::builder::tsweep(&*face, truck_modeling::Vector3::unit_z() * height);

    return_model(Arc::new(solid), env)
}

#[lisp_fn]
fn sandbox(_: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    // Creating a box using OpenCascade
    let box_maker = BRepPrimAPI_MakeBox::new(
        OCPoint3::new(0.0, 0.0, 0.0),
        OCPoint3::new(10.0, 10.0, 10.0),
    );
    let box_shape = box_maker.shape();
    return_model(Model::OCShape(box_shape), env)
}

/// Create a box primitive with OpenCascade
///
/// # Lisp Usage
/// `(oc-box x y z width height depth)`
///
/// # Examples
/// `(oc-box 0 0 0 10 20 30)` - creates a box at origin with dimensions 10x20x30
///
/// # Returns
/// A box solid model
#[lisp_fn]
fn oc_box(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 6)?;

    // Extract parameters
    let x = extract::number(args[0].as_ref())?;
    let y = extract::number(args[1].as_ref())?;
    let z = extract::number(args[2].as_ref())?;
    let width = extract::number(args[3].as_ref())?;
    let height = extract::number(args[4].as_ref())?;
    let depth = extract::number(args[5].as_ref())?;

    // Create a box using OpenCascade
    let box_maker = BRepPrimAPI_MakeBox::new(
        OCPoint3::new(x, y, z),
        OCPoint3::new(x + width, y + height, z + depth),
    );

    // Get the resulting solid shape
    let box_shape = box_maker.shape();

    // Return as a shape model
    return_model(Model::OCShape(box_shape), env)
}

/// Create a point using OpenCascade
///
/// # Lisp Usage
/// `(oc-point x y z)`
///
/// # Examples
/// `(oc-point 1 2 3)` - creates a point at coordinates (1,2,3)
///
/// # Returns
/// A point model
#[lisp_fn]
fn oc_point(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 3)?;

    // Extract coordinates
    let x = extract::number(args[0].as_ref())?;
    let y = extract::number(args[1].as_ref())?;
    let z = extract::number(args[2].as_ref())?;

    // Create an OpenCascade point
    let point = OCPoint3::new(x, y, z);

    // Return as a point model
    return_model(Model::OCPoint3(point), env)
}

/// Fuse (union) two shapes with OpenCascade
///
/// # Lisp Usage
/// `(oc-fuse shape1 shape2)`
///
/// # Examples
/// `(oc-fuse (oc-box 0 0 0 10 10 10) (oc-box 5 5 5 10 10 10))` - creates a union of two overlapping boxes
///
/// # Returns
/// The union of two shapes
#[lisp_fn]
fn oc_fuse(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 2)?;

    // Extract the shapes from arguments
    let shape1 = extract::oc_shape(args[0].as_ref(), &env)?;
    let shape2 = extract::oc_shape(args[1].as_ref(), &env)?;

    // Perform the boolean union operation
    let fuse = BRepAlgoAPI_Fuse::new(&shape1, &shape2);
    let result = fuse.shape();

    // Return the resulting shape
    return_model(Model::OCShape(result), env)
}

/// Find the common (intersection) of two shapes with OpenCascade
///
/// # Lisp Usage
/// `(oc-common shape1 shape2)`
///
/// # Examples
/// `(oc-common (oc-box 0 0 0 10 10 10) (oc-box 5 5 5 10 10 10))` - creates the intersection of two overlapping boxes
///
/// # Returns
/// The intersection of two shapes
#[lisp_fn]
fn oc_common(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 2)?;

    // Extract the shapes from arguments
    let shape1 = extract::oc_shape(args[0].as_ref(), &env)?;
    let shape2 = extract::oc_shape(args[1].as_ref(), &env)?;

    // Perform the boolean intersection operation
    let common = BRepAlgoAPI_Common::new(&shape1, &shape2);
    let result = common.shape();

    // Return the resulting shape
    return_model(Model::OCShape(result), env)
}

/// Cut one shape from another with OpenCascade
///
/// # Lisp Usage
/// `(oc-cut shape1 shape2)`
///
/// # Examples
/// `(oc-cut (oc-box 0 0 0 10 10 10) (oc-box 2 2 2 6 6 6))` - creates a box with a smaller box cut out of it
///
/// # Returns
/// The result of cutting shape2 from shape1
#[lisp_fn]
fn oc_cut(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 2)?;

    // Extract the shapes from arguments
    let shape1 = extract::oc_shape(args[0].as_ref(), &env)?;
    let shape2 = extract::oc_shape(args[1].as_ref(), &env)?;

    // Perform the boolean subtraction operation
    let cut = BRepAlgoAPI_Cut::new(&shape1, &shape2);
    let result = cut.shape();

    // Return the resulting shape
    return_model(Model::OCShape(result), env)
}

#[allow(dead_code)]
fn debug(wire: &truck_modeling::Wire) {
    let vertex_format = VertexDisplayFormat::Full;
    let _edge_format = EdgeDisplayFormat::Full { vertex_format };
    let wire_format = WireDisplayFormat::VerticesList { vertex_format };
    println!("{:?}", wire.display(wire_format));
}

/// Create the intersection of two or more solid models using Truck (Legacy)
///
/// # Lisp Usage
/// `(and solid1 solid2 ...)`
///
/// # Examples
/// `(and (linear-extrude (circle 0 0 5) 10) (linear-extrude (circle 5 0 5) 10))` - intersection of two cylinders
///
/// # Returns
/// A solid model representing the intersection
#[lisp_fn]
fn and(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 2..).map_err(|e| format!("and: {}", e))?;

    let solids = args
        .iter()
        .map(|expr| extract::solid(expr.as_ref(), &env))
        .collect::<Result<Vec<_>, String>>()?;

    if solids.len() < 2 {
        return Err("and: expected at least 2 solid models".to_string());
    }

    let mut result = (*solids[0]).clone();

    for solid in &solids[1..] {
        result = match truck_shapeops::and(&result, &**solid, 0.01) {
            Some(solid) => solid,
            None => return Err(format!("Boolean AND operation failed")),
        };
    }
    return_model(Arc::new(result), env)
}

/// Create the union of two or more solid models using Truck (Legacy)
///
/// # Lisp Usage
/// `(or solid1 solid2 ...)`
///
/// # Examples
/// `(or (linear-extrude (circle 0 0 5) 10) (linear-extrude (circle 5 0 5) 10))` - union of two cylinders
///
/// # Returns
/// A solid model representing the union
#[lisp_fn]
fn or(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 2..).map_err(|e| format!("or: {}", e))?;

    let solids = args
        .iter()
        .map(|expr| extract::solid(expr.as_ref(), &env))
        .collect::<Result<Vec<_>, String>>()?;

    if solids.len() < 2 {
        return Err("or: expected at least 2 solid models".to_string());
    }

    let mut result = (*solids[0]).clone();

    for solid in &solids[1..] {
        result = match truck_shapeops::or(&result, &**solid, 0.01) {
            Some(solid) => solid,
            None => return Err(format!("Boolean OR operation failed")),
        };
    }
    return_model(Arc::new(result), env)
}

/// Subtract one or more solids from the first solid using Truck (Legacy)
///
/// # Lisp Usage
/// `(not base_solid solid_to_subtract1 solid_to_subtract2 ...)`
///
/// # Examples
/// `(not (linear-extrude (circle 0 0 10) 10) (linear-extrude (circle 0 0 5) 20))` - cylinder with a hole
///
/// # Returns
/// A solid model representing the difference
#[lisp_fn]
fn not(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 1)?;

    let solid = extract::solid(args[0].as_ref(), &env)?;

    let mut result: Solid = (*solid).clone();
    result.not();

    return_model(Arc::new(result), env)
}

// These functions are duplicated elsewhere in this file, so they're commented out for now.
/*
/// Create the intersection of two shapes using OpenCascade
///
/// # Lisp Usage
/// `(oc-common shape1 shape2)`
///
/// # Returns
/// A shape model representing the intersection
#[lisp_fn]
fn oc_common(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 2)?;

    // Extract the shapes from arguments
    let shape1 = extract::oc_shape(args[0].as_ref(), &env)?;
    let shape2 = extract::oc_shape(args[1].as_ref(), &env)?;

    // Perform the boolean intersection operation
    let common = BRepAlgoAPI_Common::new(&shape1, &shape2);
    let result = common.shape();

    // Return the resulting shape
    return_model(Model::OCShape(result), env)
}

/// Subtract one shape from another using OpenCascade
///
/// # Lisp Usage
/// `(oc-cut shape1 shape2)`
///
/// # Returns
/// A shape model representing the difference (shape1 - shape2)
#[lisp_fn]
fn oc_cut(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 2)?;

    // Extract the shapes from arguments
    let shape1 = extract::oc_shape(args[0].as_ref(), &env)?;
    let shape2 = extract::oc_shape(args[1].as_ref(), &env)?;

    // Perform the boolean subtraction operation
    let cut = BRepAlgoAPI_Cut::new(&shape1, &shape2);
    let result = cut.shape();

    // Return the resulting shape
    return_model(Model::OCShape(result), env)
}
*/

/// Translate a model by a given vector (Legacy Truck version)
///
/// # Lisp Usage
/// `(translate model dx dy dz)`
///
/// # Examples
/// `(translate (p 0 0 0) 1 2 3)` - translates the point to (1, 2, 3)
///
/// # Returns
/// The translated model
#[lisp_fn]
fn translate(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 4)?;

    let model_id = match args[0].as_ref() {
        Expr::Model { id, .. } => *id,
        _ => return Err("translate: expected a model as the first argument".to_string()),
    };

    let dx = extract::number(eval(args[1].clone(), env.clone())?.as_ref())?;
    let dy = extract::number(eval(args[2].clone(), env.clone())?.as_ref())?;
    let dz = extract::number(eval(args[3].clone(), env.clone())?.as_ref())?;

    let translation = truck_modeling::Vector3::new(dx, dy, dz);
    let model = env
        .lock()
        .unwrap()
        .get_model(model_id)
        .ok_or_else(|| format!("Model with id {} not found", model_id))?;
    let translated_model = match model.as_ref() {
        Model::Point3(p) => Ok(Model::Point3(p + translation)),
        Model::Wire(w) => Ok(Model::Wire(Arc::new(translated(w.as_ref(), translation)))),
        Model::Edge(e) => Ok(Model::Edge(Arc::new(translated(e.as_ref(), translation)))),
        Model::Face(f) => Ok(Model::Face(Arc::new(translated(f.as_ref(), translation)))),
        Model::Solid(s) => Ok(Model::Solid(Arc::new(translated(s.as_ref(), translation)))),
        Model::Mesh(_) => Err("nothing can be done on mesh".to_string()),
        _ => Err(
            "translate: Expected a Truck model. Use oc-translate for OpenCascade models."
                .to_string(),
        ),
    }?;
    return_model(translated_model, env)
}

/// Translate an OpenCascade shape
///
/// # Lisp Usage
/// `(oc-translate shape dx dy dz)`
///
/// # Examples
/// `(oc-translate (oc-box 0 0 0 10 10 10) 5 5 5)` - translates the box by (5,5,5)
///
/// # Returns
/// The translated shape
#[lisp_fn]
fn oc_translate(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 4)?;

    // Extract the shape and translation values
    let shape = extract::oc_shape(args[0].as_ref(), &env)?;
    let dx = extract::number(eval(args[1].clone(), env.clone())?.as_ref())?;
    let dy = extract::number(eval(args[2].clone(), env.clone())?.as_ref())?;
    let dz = extract::number(eval(args[3].clone(), env.clone())?.as_ref())?;

    // Create a transformation
    let mut transform = gp_Trsf::new();
    transform.set_translation(OCPoint3::new(0.0, 0.0, 0.0), OCPoint3::new(dx, dy, dz));

    // Apply the transformation to the shape
    let transformation = BRepBuilderAPI_Transform::new(&shape, &transform, true);
    let result = transformation.shape();

    // Return the transformed shape
    return_model(Model::OCShape(result), env)
}

/// Rotate an OpenCascade shape around an axis
///
/// # Lisp Usage
/// `(oc-rotate shape center-x center-y center-z axis-x axis-y axis-z angle)`
///
/// # Examples
/// `(oc-rotate (oc-box 0 0 0 10 10 10) 0 0 0 0 0 1 90)` - rotates the box 90 degrees around the Z-axis
///
/// # Returns
/// The rotated shape
#[lisp_fn]
fn oc_rotate(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 8)?;

    // Extract the shape
    let shape = extract::oc_shape(args[0].as_ref(), &env)?;

    // Extract center point
    let center_x = extract::number(args[1].as_ref())?;
    let center_y = extract::number(args[2].as_ref())?;
    let center_z = extract::number(args[3].as_ref())?;

    // Extract axis of rotation
    let axis_x = extract::number(args[4].as_ref())?;
    let axis_y = extract::number(args[5].as_ref())?;
    let axis_z = extract::number(args[6].as_ref())?;

    // Extract angle in degrees
    let angle_degrees = extract::number(args[7].as_ref())?;

    // Create a rotation transformation
    let mut transform = gp_Trsf::new();
    let center_point = OCPoint3::new(center_x, center_y, center_z);
    // Using Direction constructor with correct method name for opencascade-sys
    let axis_direction = Direction::create(axis_x, axis_y, axis_z);

    // Convert angle to radians
    let angle_radians = angle_degrees * std::f64::consts::PI / 180.0;

    // Set up rotation
    // Using OCPoint3 directly as it is already gp_Pnt
    transform.set_rotation(center_point, axis_direction, angle_radians);

    // Apply the transformation to the shape
    let transformation = BRepBuilderAPI_Transform::new(&shape, &transform, true);
    let result = transformation.shape();

    // Return the rotated shape
    return_model(Model::OCShape(result), env)
}

/// Rotate a model around a given axis by a specified angle (Legacy Truck version)
///
/// # Lisp Usage
/// `(rotate model axis_x axis_y axis_z angle)`
///
/// # Examples
/// `(rotate (p 1 2 3) (p 0 0 0) 'z 90)` - rotates the point 90 degrees around the Z-axis
///
/// # Returns
/// The rotated model
#[lisp_fn]
fn rotate(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 4)?;

    let model_id = match args[0].as_ref() {
        Expr::Model { id, .. } => *id,
        _ => return Err("rotate: expected a model as the first argument".to_string()),
    };

    let origin = extract::point3(eval(args[1].clone(), env.clone())?.as_ref(), &env.clone())?;
    let axis = match args[2].as_ref() {
        Expr::Symbol { name, .. } => match name.as_str() {
            "x" => truck_modeling::Vector3::unit_x(),
            "y" => truck_modeling::Vector3::unit_y(),
            "z" => truck_modeling::Vector3::unit_z(),
            _ => return Err("rotate: expected 'x', 'y', or 'z' as the axis".to_string()),
        },
        _ => return Err("rotate: expected a symbol for the axis".to_string()),
    };
    let angle: f64 = extract::number(args[3].as_ref())?;

    let angle_deg = truck_polymesh::Deg(angle);
    let model = env
        .lock()
        .unwrap()
        .get_model(model_id)
        .ok_or_else(|| format!("Model with id {} not found", model_id))?;
    let rotated_model = match model.as_ref() {
        Model::Point3(_) => Err("TODO: cannot rotate a point".to_string()),
        Model::Wire(w) => Ok(Model::Wire(Arc::new(rotated(
            w.as_ref(),
            origin,
            axis,
            angle_deg.into(),
        )))),
        Model::Edge(e) => Ok(Model::Edge(Arc::new(rotated(
            e.as_ref(),
            origin,
            axis,
            angle_deg.into(),
        )))),
        Model::Face(f) => Ok(Model::Face(Arc::new(rotated(
            f.as_ref(),
            origin,
            axis,
            angle_deg.into(),
        )))),
        Model::Solid(s) => Ok(Model::Solid(Arc::new(rotated(
            s.as_ref(),
            origin,
            axis,
            angle_deg.into(),
        )))),
        Model::Mesh(_) => Err("nothing can be done on mesh".to_string()),
        _ => {
            Err("rotate: Expected a Truck model. Use oc-rotate for OpenCascade models.".to_string())
        }
    }?;

    return_model(rotated_model, env)
}

// This function is a duplicate, commenting out for now.
/*
/// Rotate an OpenCascade shape around an axis
///
/// # Lisp Usage
/// `(oc-rotate shape center-x center-y center-z axis-x axis-y axis-z angle)`
///
/// # Examples
/// `(oc-rotate (oc-box 0 0 0 10 10 10) 0 0 0 0 0 1 90)` - rotates the box 90 degrees around the Z-axis
///
/// # Returns
/// The rotated shape
#[lisp_fn]
fn oc_rotate(args: &[Arc<Expr>], env: Arc<Mutex<Env>>) -> Result<Arc<Expr>, String> {
    assert_arg_count(args, 8)?;

    // Extract the shape
    let shape = extract::oc_shape(args[0].as_ref(), &env)?;

    // Extract center point
    let center_x = extract::number(args[1].as_ref())?;
    let center_y = extract::number(args[2].as_ref())?;
    let center_z = extract::number(args[3].as_ref())?;

    // Extract axis of rotation
    let axis_x = extract::number(args[4].as_ref())?;
    let axis_y = extract::number(args[5].as_ref())?;
    let axis_z = extract::number(args[6].as_ref())?;

    // Extract angle in degrees
    let angle_degrees = extract::number(args[7].as_ref())?;

    // Create a rotation transformation
    let mut transform = gp_Trsf::new();
    let center_point = OCPoint3::new(center_x, center_y, center_z);
    // Using Direction constructor with correct method name for opencascade-sys
    let axis_direction = Direction::create(axis_x, axis_y, axis_z);

    // Convert angle to radians
    let angle_radians = angle_degrees * std::f64::consts::PI / 180.0;

    // Set up rotation
    // Using OCPoint3 directly as it is already gp_Pnt
    transform.set_rotation(center_point, axis_direction, angle_radians);

    // Apply the transformation to the shape
    let transformation = BRepBuilderAPI_Transform::new(&shape, &transform, true);
    let result = transformation.shape();

    // Return the rotated shape
    return_model(Model::OCShape(result), env)
}
*/
