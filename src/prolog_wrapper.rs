use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use bevy_async_ecs::AsyncWorld;
use derived_deref::{Deref, DerefMut};

use crate::events::{GeneratePreviewRequest, PreviewGenerated};
use prolog::mock::mock_generate_mesh;

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
    mut ev_requests: EventReader<GeneratePreviewRequest>,
    async_world: Res<AsyncWorldRes>,
) {
    for req in ev_requests.read() {
        let async_world = async_world.clone();
        let request_id = req.request_id;
        let query = req.query.clone();
        AsyncComputeTaskPool::get()
            .spawn(async move {
                let mesh = mock_generate_mesh();
                async_world
                    .apply(move |world: &mut World| {
                        world.send_event(PreviewGenerated {
                            request_id,
                            query,
                            mesh,
                        });
                    })
                    .await;
            })
            .detach();
    }
}
