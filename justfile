set shell := ["nu", "-c"]

default:
    cargo run

build:
    cargo build
    cd tree-sitter-cadhr-lang; tree-sitter generate
    cd tree-sitter-cadhr-lang; cc -shared -o cadhr_lang.so -I src src/parser.c -O2
    cp tree-sitter-cadhr-lang/cadhr_lang.so ~/.local/share/nvim/site/parser/cadhr_lang.so
    cd cadhr-lsp; cargo build --release
