use bevy::prelude::*;
use std::thread;
use std::time::Duration;

/// Mock async generator: waits ~1 second and returns a simple cube mesh.
pub async fn mock_generate_mesh() -> Mesh {
    // Simulate heavy Prolog + generation work
    thread::sleep(Duration::from_secs(1));
    Mesh::from(Cuboid::from_size(Vec3::splat(1.0)))
}

