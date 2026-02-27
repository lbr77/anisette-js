# anisette-js

Apple Anisette authentication in browser via WebAssembly. Emulates ARM64 Android binaries to generate Anisette headers locally — no third-party servers required.

## Features

- **Local execution**: All computation happens in your browser or Node.js process
- **WASM-based**: Uses Unicorn Engine compiled to WebAssembly for ARM64 emulation
- **High-level JS/TS API**: Simple async interface, handles provisioning automatically
- **Single-file bundle**: Distribute as one `.js` + one `.wasm` file

## Prerequisites

- Rust nightly (for building the WASM module)
- Emscripten SDK
- Bun (for bundling the TypeScript API)

Android library blobs (`libstoreservicescore.so`, `libCoreADI.so`) are not included. Extract them from an Apple Music APK or obtain separately.

## Build

```bash
# Clone and build custom Unicorn fork
git clone https://github.com/lbr77/unicorn.git
cd unicorn && git checkout tci-emscripten

# Build everything (WASM + TS API bundle)
bash script/build-glue.sh

# Or build just the JS bundle (WASM already built)
npm run build:js
```

Output files in `dist/`:
- `anisette.js` — bundled TS API + glue (single file)
- `anisette_rs.node.wasm` — WASM binary (required alongside `.js`)

## Usage

### Node.js

```javascript
import { Anisette, loadWasm } from "./dist/anisette.js";
import fs from "node:fs/promises";

const wasmModule = await loadWasm();

const storeservices = new Uint8Array(await fs.readFile("libstoreservicescore.so"));
const coreadi = new Uint8Array(await fs.readFile("libCoreADI.so"));

const anisette = await Anisette.fromSo(storeservices, coreadi, wasmModule);

if (!anisette.isProvisioned) {
  await anisette.provision();
}

const headers = await anisette.getData();
console.log(headers["X-Apple-I-MD"]);
```

Run the example:

```bash
node example/anisette-api.mjs libstoreservicescore.so libCoreADI.so ./anisette/
```

### Browser

For browser usage, use the web-targeted WASM build (`anisette_rs.js` / `.wasm`) and import directly:

```javascript
import ModuleFactory from "./anisette_rs.js";

const wasmModule = await ModuleFactory({
  locateFile: (f) => f.endsWith(".wasm") ? "./anisette_rs.wasm" : f
});

// Use WasmBridge for low-level access, or wrap with the TS API
```

## API Reference

### `Anisette`

Main class for generating Anisette headers.

**Static methods:**

- `Anisette.fromSo(storeservicescore, coreadi, wasmModule, options?)` — Initialize from library blobs
- `Anisette.fromSaved(ss, ca, deviceJson, adiPb, wasmModule, options?)` — Restore a saved session

**Instance properties:**

- `isProvisioned: boolean` — Whether the device is provisioned

**Instance methods:**

- `provision()` — Run Apple provisioning flow
- `getData(): Promise<AnisetteHeaders>` — Generate Anisette headers
- `getDeviceJson(): Uint8Array` — Serialize device config for persistence

### `loadWasm()`

Loads the WASM module. In Node.js, resolves `.wasm` path relative to the bundle location.

```javascript
import { loadWasm } from "./dist/anisette.js";
const wasmModule = await loadWasm();
```

## Architecture

- **Rust/WASM core** (`src/`): Emulator, ADI wrapper, provisioning protocol
- **TypeScript API** (`js/src/`): High-level wrapper around WASM exports
- **Emscripten glue**: Bridges JS and WASM memory, handles VFS

Key modules:
- `adi.rs` — ADI (Apple Device Identity) provisioning and OTP
- `emu.rs` — Unicorn-based ARM64 emulator
- `exports.rs` — C FFI exports for WASM
- `js/src/anisette.ts` — Main `Anisette` class
- `js/src/wasm-bridge.ts` — Low-level WASM memory management

## Credits

- [pyprovision-uc](https://github.com/JayFoxRox/pyprovision-uc)
- [Anisette.py](https://github.com/malmeloo/Anisette.py)
- [omnisette-server](https://github.com/SideStore/omnisette-server)
- [unicorn](https://github.com/petabyt/unicorn/tree/tci-emscripten)


## Known Issue:

when requiring Otp for second time there will be a "WRITE UNMAPPED" error which could be avoided by initalizing onemoretime...