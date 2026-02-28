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

const wasmModule = await loadWasm({ printErr: () => {}});

const storeservices = new Uint8Array(await fs.readFile(storeservicesPath));
const coreadi = new Uint8Array(await fs.readFile(coreadiPath));

const readOptional = (p) => fs.readFile(p).then((b) => new Uint8Array(b)).catch(() => {console.warn(`Optional file not found: ${p}`); return null; });
const [adiPb, deviceJsonBytes] = await Promise.all([
  readOptional(path.join(libraryPath, "adi.pb")),
  readOptional(path.join(libraryPath, "device.json")),
]);

const anisette = await Anisette.fromSo(storeservices, coreadi, wasmModule, {
  init: { libraryPath, adiPb, deviceJsonBytes },
});

if (!anisette.isProvisioned) {
  console.log("Device not provisioned — running provisioning...");
  await anisette.provision();
  await fs.mkdir(libraryPath, { recursive: true });
  await fs.writeFile(path.join(libraryPath, "adi.pb"), anisette.getAdiPb());
  await fs.writeFile(path.join(libraryPath, "device.json"), anisette.getDeviceJson());
  console.log("Provisioning complete.");
} else {
  console.log("Device already provisioned.");
}

const headers = await anisette.getData();
console.log(JSON.stringify(headers, null, 2));


console.log(JSON.stringify(await anisette.getData(), null, 2));