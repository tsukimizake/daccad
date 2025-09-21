use manifold_rs::{Manifold, Mesh};
use std::thread;
use std::time::Duration;

// Simple synchronous mock: waits ~1s and returns a cube mesh (manifold-rs Mesh).
// Normals are calculated via manifold-rs and embedded in vertex properties.
pub fn mock_generate_mesh() -> Mesh {
    thread::sleep(Duration::from_secs(1));
    // 1x1x1 cube. Calculate normals in property channel 0 (immediately after XYZ).
    let cube = Manifold::cube(1.0, 1.0, 1.0);
    let with_normals = cube.calculate_normals(0, 30.0);
    with_normals.to_mesh()
}
