# AGENTS.md

This file provides guidance to Every Agents when working with code in this repository.

## Overview

This crate provides Apple Anisette authentication in browser via WebAssembly. It uses a custom Unicorn Engine fork (https://github.com/lbr77/unicorn/tree/tci-emscripten) to emulate ARM64 Android binaries (`libstoreservicescore.so` + `libCoreADI.so`) for generating Anisette headers.

**Note**: The Android library blobs (`libstoreservicescore.so`, `libCoreADI.so`) are not included in this repository. Extract them from an APK or obtain separately.

## Build Commands

### Prerequisites

- Rust nightly (edition 2024)
- Emscripten SDK (for WASM builds)

### Setup Unicorn Engine

Clone the custom Unicorn repository and checkout to the `tci-emscripten` branch:

```bash
git clone https://github.com/lbr77/unicorn.git
cd unicorn && git checkout tci-emscripten
```

Then build Unicorn for Emscripten:

```bash
bash script/rebuild-unicorn.sh
```

The rebuild script handles:
- Running `emcmake cmake` with appropriate flags
- Building only `arm` and `aarch64` architectures
- Using static archives (`-DUNICORN_LEGACY_STATIC_ARCHIVE=ON`)

### Build WASM Glue

```bash
bash script/build-glue.sh           # Debug build
bash script/build-glue.sh --release # Release build
```

Outputs:
- `test/dist/anisette_rs.js` / `.wasm` (web)
- `test/dist/anisette_rs.node.js` / `.wasm` (Node.js)
- Copies to `../../frontend/public/anisette/`

### Run Native Example

```bash
cargo run --example anisette -- <libstoreservicescore.so> <libCoreADI.so> [library_path] [dsid] [apple_root_pem]
```

### Run Node.js Example

```bash
node example/run-node.mjs <libstoreservicescore.so> <libCoreADI.so> [library_path] [dsid] [identifier]
```

## Architecture

### Core Modules

- **`adi.rs`**: ADI (Apple Device Identity) wrapper — provisioning, OTP requests
- **`emu.rs`**: Unicorn-based ARM64 emulator core — library loading, symbol resolution, function calls
- **`exports.rs`**: C FFI exports for WASM — `anisette_*` functions
- **`device.rs`**: Device identity management — UUIDs, identifiers, persistence
- **`idbfs.rs`**: IndexedDB filesystem integration for Emscripten
- **`provisioning.rs`** / **`provisioning_wasm.rs`**: Apple provisioning protocol

### Emulator Memory Layout

- Libraries mapped to import address space with stub hooks
- Stack, heap, and return addresses pre-allocated
- Import hooks dispatch to runtime stubs

### Public API (exports.rs)

- `anisette_init_from_blobs` — Initialize from library blobs
- `anisette_is_machine_provisioned` — Check provisioning state
- `anisette_start_provisioning` / `anisette_end_provisioning` — Provisioning flow
- `anisette_request_otp` — Generate OTP + machine ID headers

### Data Flow

1. Load Android `libstoreservicescore.so` + `libCoreADI.so`
2. Initialize device identity (`device.json`)
3. Provision with Apple (if needed)
4. Request OTP → `X-Apple-I-MD` + `X-Apple-I-MD-M` headers
