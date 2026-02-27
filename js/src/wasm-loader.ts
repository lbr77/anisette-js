// Loads the Emscripten WASM glue bundled alongside this file.
// The .wasm binary is resolved relative to this JS file at runtime.

// @ts-ignore — glue file is generated, no types available
import ModuleFactory from "../../dist/anisette_rs.node.js";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import path from "node:path";

// Resolve the .wasm file next to the bundled output JS
function resolveWasmPath(outputFile: string): string {
  // __filename of the *bundled* output — bun sets import.meta.url correctly
  const dir = path.dirname(fileURLToPath(import.meta.url));
  return path.join(dir, outputFile);
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export async function loadWasm(): Promise<any> {
  return ModuleFactory({
    locateFile(file: string) {
      if (file.endsWith(".wasm")) {
        return resolveWasmPath("anisette_rs.node.wasm");
      }
      return file;
    },
  });
}
