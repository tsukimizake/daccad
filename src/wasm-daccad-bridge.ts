import init, { DaccadEngine } from '../pkg/wasm_daccad_engine.js';
import type { FromElmMessage, ToElmMessage, ModelId } from '../pkg/wasm_daccad_engine.js';
import * as Manifold from 'manifold-3d';
import type { Manifold as ManifoldType } from 'manifold-3d';

interface MeshData {
  vertices: Float32Array;
  faces: Uint32Array;
  vertexCount: number;
  faceCount: number;
}

export interface ElmApp {
  ports?: {
    fromElm?: {
      subscribe: (callback: (message: unknown) => void) => void;
    };
    toElm?: {
      send: (data: unknown) => void;
    };
  };
}

declare global {
  interface Window {
    manifoldBridge: {
      createCube: (width: number, height: number, depth: number) => ManifoldType;
      createCylinder: (height: number, radius: number, segments?: number) => ManifoldType;
      getMeshData: (manifold: ManifoldType) => MeshData | null;
    };
  }
}

type DaccadBridgeState =
  | { status: 'uninitialized' }
  | {
    status: 'initialized';
    engine: DaccadEngine;
    manifold: Manifold.ManifoldToplevel;
    elmApp: ElmApp | null;
  };

let state: DaccadBridgeState = { status: 'uninitialized' };


export const initDaccadBridge = async (): Promise<void> => {
  if (state.status === 'initialized') return;

  await init();
  const engine = new DaccadEngine();

  const Module = await Manifold.default();

  if (Module.setup) {
    Module.setup();
  }

  console.log('Manifold instance methods:', Object.getOwnPropertyNames(Module));
  console.log('Manifold prototype methods:', Object.getOwnPropertyNames(Object.getPrototypeOf(Module)));

  (window as any).manifoldBridge = {
    createCube: (width: number, height: number, depth: number) => {
      return Module.Manifold.cube([width, height, depth]);
    },
    createCylinder: (height: number, radius: number, segments?: number) => {
      return Module.Manifold.cylinder(height, radius, radius, segments || 64);
    },
    getMeshData: (manifold: ManifoldType): MeshData | null => {
      if (manifold && manifold.getMesh) {
        const mesh = manifold.getMesh();
        return {
          vertices: mesh.vertProperties,
          faces: mesh.triVerts,
          vertexCount: mesh.vertProperties.length / 3,
          faceCount: mesh.triVerts.length / 3
        };
      }
      return null;
    }
  };

  state = {
    status: 'initialized',
    engine,
    manifold: Module,
    elmApp: null
  };
  console.log('Daccad Bridge initialized successfully');
};


export const clearEnv = (): void => {
  if (state.status !== 'initialized') {
    throw new Error('Bridge not initialized');
  }
  state.engine.clear_env();
};

export const getManifold = (): Manifold.ManifoldToplevel | null => {
  return state.status === 'initialized' ? state.manifold : null;
};

export const createCube = (width: number, height: number, depth: number): ManifoldType => {
  if (state.status !== 'initialized') {
    throw new Error('Manifold not initialized');
  }
  return state.manifold.Manifold.cube([width, height, depth]);
};

export const createCylinder = (height: number, radius: number, segments?: number): ManifoldType => {
  if (state.status !== 'initialized') {
    throw new Error('Manifold not initialized');
  }
  return state.manifold.Manifold.cylinder(height, radius, radius, segments || 64);
};

export const union = (a: ManifoldType, b: ManifoldType): ManifoldType => {
  return a.add(b);
};

export const subtract = (a: ManifoldType, b: ManifoldType): ManifoldType => {
  return a.subtract(b);
};

export const intersect = (a: ManifoldType, b: ManifoldType): ManifoldType => {
  return a.intersect(b);
};

export const translate = (obj: ManifoldType, x: number, y: number, z: number): ManifoldType => {
  return obj.translate([x, y, z]);
};

export const rotate = (obj: ManifoldType, x: number, y: number, z: number): ManifoldType => {
  return obj.rotate([x, y, z]);
};

export const scale = (obj: ManifoldType, x: number, y: number, z: number): ManifoldType => {
  return obj.scale([x, y, z]);
};

export const getMeshData = (manifold: ManifoldType): MeshData => {
  const mesh = manifold.getMesh();
  const vertices = mesh.vertProperties;
  const faces = mesh.triVerts;

  return {
    vertices,
    faces,
    vertexCount: vertices.length / 3,
    faceCount: faces.length / 3
  };
};

export const getStlBytes = (modelId: ModelId): Uint8Array | null => {
  if (state.status !== 'initialized') {
    return null;
  }

  const bytes = state.engine.get_mesh_stl_bytes(modelId);
  return bytes ? new Uint8Array(bytes) : null;
};

// Elm integration functions
const sendToElm = (data: ToElmMessage): void => {
  if (state.status === 'initialized' && state.elmApp?.ports?.toElm) {
    state.elmApp.ports.toElm.send(data);
  }
};

const handleFromElmMessage = async (message: FromElmMessage): Promise<void> => {
  if (state.status !== 'initialized') {
    throw new Error('Bridge not initialized');
  }

  // Handle LoadFile via Tauri
  if (message.type === 'LoadFile') {
    try {
      // Check if we're running in a Tauri context
      if (!("__TAURI__" in window)) {
        throw new Error('File loading is only available in Tauri environment');
      }
      const { invoke } = await import('@tauri-apps/api/core');
      const content = await invoke<string>('read_file', { path: message.file_path });
      sendToElm({
        type: 'FileLoaded',
        path: message.file_path,
        content: content
      });
    } catch (error) {
      console.error('Failed to load file:', error);
      sendToElm({
        type: 'FileLoadError',
        error: `Failed to load file: ${error}`
      });
    }
    return;
  }

  // Pass other messages directly to rust for processing
  try {
    const result = state.engine.handle_message(JSON.stringify(message));
    sendToElm(JSON.parse(result) as ToElmMessage);
  } catch (error) {
    console.error('Message handling failed:', error);
    sendToElm({
      type: 'Error',
      message: `Message handling failed: ${error}`
    });
  }
};

const setupPorts = (elmApp: ElmApp): void => {
  if (elmApp.ports?.fromElm) {
    elmApp.ports.fromElm.subscribe((x) => handleFromElmMessage(x as FromElmMessage));
  }
};

export const initWasmElmIntegration = async (elmApp: ElmApp): Promise<void> => {
  setupPorts(elmApp);

  try {
    await initDaccadBridge();
    if (state.status === 'initialized') {
      state.elmApp = elmApp;
    }
    console.log('WASM initialized successfully');
  } catch (error) {
    console.error('Failed to initialize WASM:', error);
    sendToElm({
      type: 'Error',
      message: `Initialization failed: ${error}`
    });
  }
};
