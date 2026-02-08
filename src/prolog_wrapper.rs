use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::tasks::AsyncComputeTaskPool;
use bevy::{mesh::Indices, prelude::*};
use bevy_async_ecs::AsyncWorld;
use derived_deref::{Deref, DerefMut};

use crate::events::{GeneratePreviewRequest, PreviewGenerated};
use manifold_rs::Mesh as RsMesh;
use prolog::manifold_bridge::generate_mesh_from_term;
use prolog::parse::{database, query as parse_query};
use prolog::term_rewrite::execute;

#[derive(Resource, Clone, Deref, DerefMut)]
struct AsyncWorldRes(AsyncWorld);

pub struct PrologPlugin;

impl Plugin for PrologPlugin {
    fn build(&self, app: &mut App) {
        // Initialize AsyncWorld handle for this app
        app.add_systems(Startup, init_async_world);
        // System: consume requests and spawn workers; results applied via AsyncWorld
        app.add_systems(Update, consume_requests);
    }
}

// Initialize bevy-async-ecs and store AsyncWorld as a Resource
fn init_async_world(world: &mut World) {
    let async_world = AsyncWorld::from_world(world);
    world.insert_resource(AsyncWorldRes(async_world));
}

// Listen for requests and spawn a worker for each one
fn consume_requests(
    mut ev_requests: MessageReader<GeneratePreviewRequest>,
    async_world: Res<AsyncWorldRes>,
) {
    for req in ev_requests.read() {
        let async_world = async_world.clone();
        let request_id = req.request_id;
        let db_src = req.database.clone();
        let query = req.query.clone();
        AsyncComputeTaskPool::get()
            .spawn(async move {
                // Parse query and execute term rewrite, then generate mesh
                let result = (|| -> Result<Mesh, String> {
                    // Parse the query string
                    let (_, query_terms) =
                        parse_query(&query).map_err(|e| format!("Query parse error: {:?}", e))?;

                    // Parse database
                    let mut db = database(&db_src)
                        .map_err(|e| format!("Database parse error: {:?}", e))?;

                    println!("Query terms: {:?}", query_terms);
                    println!("Database clauses: {:#?}", db);

                    // Execute term rewrite
                    let resolved = execute(&mut db, query_terms)
                        .map_err(|e| format!("Rewrite error: {}", e))?;

                    // Take the first resolved term and convert to mesh
                    let term = resolved
                        .first()
                        .ok_or_else(|| "No resolved terms".to_string())?;

                    println!("Resolved term: {:?}", term);
                    let rs_mesh: RsMesh =
                        generate_mesh_from_term(term).map_err(|e| format!("Mesh error: {}", e))?;

                    Ok(rs_mesh_to_bevy_mesh(&rs_mesh))
                })();

                match result {
                    Ok(mesh) => {
                        async_world
                            .apply(move |world: &mut World| {
                                world.write_message(PreviewGenerated {
                                    request_id,
                                    query,
                                    mesh,
                                });
                            })
                            .await;
                    }
                    Err(e) => {
                        bevy::log::error!("Failed to generate mesh: {}", e);
                    }
                }
            })
            .detach();
    }
}

// Convert a manifold-rs Mesh into a Bevy Mesh
fn rs_mesh_to_bevy_mesh(rs_mesh: &RsMesh) -> Mesh {
    // Vertices are a flat Vec<f32> with `num_props` stride (first 3 are XYZ).
    // 法線がプロパティに無い場合はpanicします（calculate_normalsを先に呼んでいることを想定）。
    let vertices = rs_mesh.vertices();
    bevy::log::info!("manifold-rs mesh has {} vertices", vertices.len());
    let stride = rs_mesh.num_props() as usize;
    bevy::log::info!("num_props (stride) = {}", stride);
    assert!(
        stride == 6,
        "manifold-rs mesh has no normals; call calculate_normals(0, ...) before to_mesh()"
    );

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(vertices.len() / stride);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(vertices.len() / stride);
    for chunk in vertices.chunks_exact(stride) {
        positions.push([chunk[0], chunk[1], chunk[2]]);
        normals.push([chunk[3], chunk[4], chunk[5]]);
    }

    // Indices are already triangles from manifold-rs
    let indices: Vec<u32> = rs_mesh.indices();

    let mut bevy_mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    bevy_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    bevy_mesh.insert_indices(Indices::U32(indices));

    // Insert normals from manifold-rs (required by assertion above)
    bevy_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);

    bevy_mesh
}