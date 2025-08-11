fn main() {
    // elm_rs generation happens via derive macros at compile time
    // The Elm types will be generated automatically in the wasm package
    println!("cargo:rerun-if-changed=src/");
}