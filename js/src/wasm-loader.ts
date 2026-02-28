// Unified WASM loader with automatic environment detection
// Uses import.meta.url + protocol detection to support both Node.js and Browser
//
// Browser (Vite/Webpack/etc):  https://... or http://... -> anisette_rs.js + .wasm
// Node.js:                     file://...                 -> anisette_rs.node.js + .wasm

// Get the base URL from import.meta.url
const MODULE_URL = new URL(import.meta.url);
const IS_NODE = MODULE_URL.protocol === "file:";

// Determine which WASM build to use based on environment
const WASM_JS_PATH = IS_NODE
  ? new URL("../dist/anisette_rs.node.js", MODULE_URL)
  : new URL("../dist/anisette_rs.js", MODULE_URL);

const WASM_BINARY_PATH = IS_NODE
  ? new URL("../dist/anisette_rs.node.wasm", MODULE_URL)
  : new URL("../dist/anisette_rs.wasm", MODULE_URL);

// Module overrides type (Emscripten module configuration)
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type EmscriptenModule = any;

export interface ModuleOverrides {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  [key: string]: any;
}

/**
 * Load the Emscripten WASM module with automatic environment detection.
 *
 * This function automatically:
 * - Detects Node.js vs Browser environment via import.meta.url protocol
 * - Loads the appropriate WASM build (node or web)
 * - Configures locateFile to find the .wasm binary
 * - Initializes the module with optional overrides
 *
 * @param moduleOverrides - Optional Emscripten module configuration overrides
 * @returns Initialized Emscripten module with all exports (_malloc, _free, _anisette_* etc.)
 *
 * @example
 * ```ts
 * // Browser (Vue/React/Next.js) and Node.js - same code!
 * const module = await loadWasmModule();
 *
 * // With custom overrides
 * const module = await loadWasmModule({
 *   print: (text: string) => console.log("WASM:", text),
 *   printErr: (text: string) => console.error("WASM Error:", text),
 * });
 * ```
 */
export async function loadWasmModule(
  moduleOverrides: ModuleOverrides = {}
): Promise<EmscriptenModule> {
  // Dynamic import of the appropriate Emscripten glue file
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const { default: ModuleFactory } = await import(/* @vite-ignore */ WASM_JS_PATH.href) as { default: (config: ModuleOverrides) => Promise<EmscriptenModule> };

  // In browser: let Emscripten use default fetch behavior
  // In Node.js: provide locateFile to resolve the .wasm path
  const config: ModuleOverrides = IS_NODE
    ? {
        ...moduleOverrides,
        locateFile: (filename: string) => {
          if (filename.endsWith(".wasm")) {
            // In Node.js, return the absolute file path
            return WASM_BINARY_PATH.pathname;
          }
          return filename;
        },
      }
    : moduleOverrides;

  return ModuleFactory(config);
}

/**
 * Convenience function to check if running in Node.js environment.
 * Uses the same protocol detection as the loader.
 */
export function isNodeEnvironment(): boolean {
  return IS_NODE;
}

/**
 * Get the resolved WASM binary path (useful for debugging).
 */
export function getWasmBinaryPath(): string {
  return IS_NODE ? WASM_BINARY_PATH.pathname : WASM_BINARY_PATH.href;
}

// Re-export for backward compatibility
export { loadWasmModule as loadWasm };
