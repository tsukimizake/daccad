// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod elm;
mod lisp;

use elm::{FromTauriCmdType, ToTauriCmdType};
use lisp::eval::assert_arg_count;
use lisp::Expr;
use std::io::Read;
use std::sync::{Arc, Mutex};

struct SharedState {
    pub code: Mutex<String>,
    pub lisp_env: Arc<Mutex<lisp::env::Env>>,
}

impl SharedState {
    fn default() -> Self {
        let lisp_env = lisp::eval::initial_env();
        lisp_env.lock().unwrap().insert(
            "load_stl".to_string(),
            Arc::new(Expr::Builtin {
                name: "load_stl".to_string(),
                fun: prim_load_stl,
            }),
        );

        Self {
            code: Mutex::default(),
            lisp_env,
        }
    }
}

fn prim_load_stl(args: &[Arc<Expr>], env: Arc<Mutex<lisp::env::Env>>) -> Result<Arc<Expr>, String> {
    if let Err(e) = assert_arg_count(args, 1) {
        return Err(e);
    }
    match args[0].as_ref() {
        Expr::String { value: path, .. } => {
            if let Ok(buf) = read_stl_file(&path) {
                if let Ok(mesh) = stl_io::read_stl(&mut std::io::Cursor::new(&buf)) {
                    let stl = Arc::new(Expr::stl(Arc::new(mesh)));
                    env.lock().unwrap().insert("stl".to_string(), stl.clone());
                    Ok(stl)
                } else {
                    Err("load_stl: failed to parse stl".to_string())
                }
            } else {
                Err("load_stl: failed to read file".to_string())
            }
        }
        _ => Err("load_stl: expected string".to_string()),
    }
}

#[tauri::command(rename_all = "snake_case")]
fn from_elm(
    window: tauri::Window,
    state: tauri::State<SharedState>,
    args: String,
) -> Result<(), String> {
    println!("to_tauri: {:?}", args);
    match serde_json::from_str(&args).unwrap() {
        ToTauriCmdType::RequestStlFile(path) => {
            if let Ok(buf) = read_stl_file(&path) {
                to_elm(window, FromTauriCmdType::StlBytes(buf));
            }
            Ok(())
        }
        ToTauriCmdType::RequestCode(path) => {
            read_code_file(window, state, &path);
            Ok(())
        }
        ToTauriCmdType::RequestEval => {
            let code = state.code.lock().unwrap().clone();
            let result = match lisp::run_file(&code, state.lisp_env.clone()) {
                Ok(expr) => FromTauriCmdType::EvalOk(expr.format()),
                Err(err) => FromTauriCmdType::EvalError(err),
            };
            to_elm(window, result);
            Ok(())
        }
    }
}

fn read_stl_file(path: &str) -> Result<Vec<u8>, String> {
    let mut input = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf: Vec<u8> = Vec::new();
    input.read_to_end(&mut buf).unwrap();
    Ok(buf)
}

fn read_code_file(window: tauri::Window, state: tauri::State<SharedState>, path: &str) {
    if let Ok(code) = std::fs::read_to_string(path) {
        let mut r = state.code.lock().unwrap();
        r.clear();
        r.push_str(&code);
        to_elm(window, FromTauriCmdType::Code(code));
    } else {
        println!("Failed to read code file");
    }
}

#[tauri::command]
fn to_elm(window: tauri::Window, cmd: FromTauriCmdType) {
    match window.emit("tauri_msg", cmd) {
        Ok(_) => println!("event sent successfully"),
        Err(e) => println!("failed to send event: {}", e),
    }
}

fn main() {
    // the target would typically be a file
    let mut target = vec![];
    // elm_rs provides a macro for conveniently creating an Elm module with everything needed
    elm_rs::export!("Bindings", &mut target, {
        encoders: [ToTauriCmdType, FromTauriCmdType],
        decoders: [ToTauriCmdType, FromTauriCmdType],
    })
    .unwrap();
    let output = String::from_utf8(target).unwrap();

    std::fs::write("../src/elm/Bindings.elm", output).unwrap();

    tauri::Builder::default()
        .manage(SharedState::default())
        .invoke_handler(tauri::generate_handler![from_elm, to_elm])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
