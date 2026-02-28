# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Rust/WASM project that emulates ARM64 Android binaries (`libstoreservicescore.so` + `libCoreADI.so`) via a custom Unicorn Engine fork to generate Apple Anisette headers in the browser or Node.js.

The Android library blobs are not included — extract from an APK or obtain separately.

## Build Commands

Must source Emscripten before building WASM:
```bash
source "/Users/libr/Desktop/Life/emsdk/emsdk_env.sh"
```

```bash
bun run build          # WASM (debug) + JS bundle
bun run release        # WASM (release) + JS bundle
bun run build:js       # JS bundle only (no WASM rebuild)
bun run build:glue     # WASM only
bun run build:unicorn  # Rebuild Unicorn (rarely needed)
```

JS bundle outputs to `dist/anisette.js`. WASM glue outputs to `dist/anisette_rs.node.{js,wasm}` and `dist/anisette_rs.{js,wasm}`.

The `js/package.json` build script also outputs to `../dist/anisette.js` directly.

## Architecture

### Rust → WASM layer (`src/`)

- `exports.rs` — all `#[no_mangle]` C FFI exports. Every new public function must also be added to `EXPORTED_FUNCTIONS` in `script/build-glue.sh` (both `WEB_EXPORTED_FUNCTIONS` and `NODE_EXPORTED_FUNCTIONS` as appropriate).
- `adi.rs` — wraps the emulated ADI library calls
- `emu.rs` — Unicorn ARM64 emulator core
- `idbfs.rs` — Emscripten IndexedDB FS integration (browser only)

### JS/TS layer (`js/src/`)

- `anisette.ts` — main `Anisette` class. **Each `getData()` call fully reinits the WASM state** (new `WasmBridge`, re-writes VFS files, re-calls `initFromBlobs`) to work around a Unicorn emulator bug that causes illegal writes on repeated use.
- `wasm-bridge.ts` — raw pointer/length marshalling to WASM exports
- `wasm-loader.ts` — thin wrapper around `ModuleFactory`; caller must pass `locateFile` via `moduleOverrides` to resolve the `.wasm` path
- `provisioning.ts` — Apple provisioning HTTP flow (fetches SPIM, sends CPIM)
- `device.ts` — loads or generates `device.json`

### Key design decisions

- `adi.pb` (provisioning state) lives in the WASM VFS. After provisioning, call `anisette.getAdiPb()` and persist it yourself — it is **not** automatically written to disk.
- `fromSo()` accepts `init.adiPb` and `init.deviceJsonBytes` to restore a previous session into the VFS before init.
- `loadWasm()` is environment-agnostic — no `node:` imports. Pass `locateFile` in `moduleOverrides`.

### Example usage

```bash
NODE_TLS_REJECT_UNAUTHORIZED=0 bun example/anisette-api.mjs \
  <libstoreservicescore.so> <libCoreADI.so> [library_path]
```
