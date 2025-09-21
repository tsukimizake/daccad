use manifold_rs::{Manifold, Mesh};
use std::thread;
use std::time::Duration;

// Simple synchronous mock: waits ~1s and returns a cube mesh (manifold-rs Mesh).
pub fn mock_generate_mesh() -> Mesh {
    thread::sleep(Duration::from_secs(1));
    // 1x1x1 cube using manifold-rs, to be converted to Bevy Mesh in the parent crate.
    let cube = Manifold::cube(1.0, 1.0, 1.0);
    cube.to_mesh()
}
