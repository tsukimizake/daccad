use std::fs::File;
use std::io::Write;

// Import types from the lib crate
use wasm_lisp::*;

fn main() {
    let mut target = Vec::new();

    elm_rs::export!("Generated", &mut target, {
        encoders: [
            ModelId,
            ValueInner,
            Evaled,
            FromElmMessage,
            ToElmMessage
        ],
        decoders: [
            ModelId,
            ValueInner,
            Evaled,
            FromElmMessage,
            ToElmMessage
        ]
    })
    .unwrap();

    let output = String::from_utf8(target).unwrap();

    let mut file = File::create("../src/elm/Generated.elm").expect("Failed to create file");
    file.write_all(output.as_bytes())
        .expect("Failed to write file");

    println!("Generated Elm types in ../../src/elm/Generated.elm");
}
