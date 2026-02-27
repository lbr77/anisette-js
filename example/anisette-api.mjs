/**
 * Example: using the high-level Anisette JS API (Node.js)
 *
 * Usage:
 *   node example/anisette-api.mjs <libstoreservicescore.so> <libCoreADI.so> [library_path]
 */

import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const bundlePath = path.join(__dirname, "..", "dist", "anisette.js");

const { Anisette, loadWasm } = await import(
  pathToFileURL(bundlePath).href
).catch(() => {
  console.error("Bundle not found. Run: npm run build:js");
  process.exit(1);
});

const args = process.argv.slice(2);
if (args.length < 2) {
  console.error(
    "usage: node example/anisette-api.mjs <libstoreservicescore.so> <libCoreADI.so> [library_path]"
  );
  process.exit(1);
}

const storeservicesPath = args[0];
const coreadiPath = args[1];
const libraryPath = args[2] ?? "./anisette/";

const wasmModule = await loadWasm();

const storeservices = new Uint8Array(await fs.readFile(storeservicesPath));
const coreadi = new Uint8Array(await fs.readFile(coreadiPath));

const anisette = await Anisette.fromSo(storeservices, coreadi, wasmModule, {
  init: { libraryPath },
});

if (!anisette.isProvisioned) {
  console.log("Device not provisioned — running provisioning...");
  await anisette.provision();
  console.log("Provisioning complete.");
} else {
  console.log("Device already provisioned.");
}

const headers = await anisette.getData();
console.log(JSON.stringify(headers, null, 2));
