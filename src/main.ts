import { Elm } from "./elm/Main.elm";
import { initWasmElmIntegration, type ElmApp } from "./wasm-lisp-bridge.ts";

// Initialize the Elm app
const app = Elm.Main.init({
  node: document.getElementById("myapp"),
}) as ElmApp;

// Initialize WASM integration
initWasmElmIntegration(app).catch(console.error);


console.log("Application initialized with WASM Lisp integration");
