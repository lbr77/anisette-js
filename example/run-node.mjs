import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const distDir = path.join(path.join(__dirname, '..'), 'dist')
const modulePath = path.join(distDir, 'anisette_rs.node.js');
const wasmPath = path.join(distDir, 'anisette_rs.node.wasm');

function usage() {
  console.log('usage: bun test/run-node.mjs <libstoreservicescore.so> <libCoreADI.so> [library_path] [dsid] [identifier] [trace_window_start]');
  console.log('note: library_path should contain adi.pb/device.json when available');
}

function allocBytes(module, bytes) {
  const ptr = module._malloc(bytes.length);
  module.HEAPU8.set(bytes, ptr);
  return ptr;
}

function allocCString(module, value) {
  if (!value) {
    return 0;
  }
  const size = module.lengthBytesUTF8(value) + 1;
  const ptr = module._malloc(size);
  module.stringToUTF8(value, ptr, size);
  return ptr;
}

function readLastError(module) {
  const ptr = module._anisette_last_error_ptr();
  const len = module._anisette_last_error_len();
  if (!ptr || !len) {
    return '';
  }
  const bytes = module.HEAPU8.subarray(ptr, ptr + len);
  return new TextDecoder('utf-8').decode(bytes);
}

function normalizeLibraryRoot(input) {
  const trimmed = input.trim();
  if (!trimmed) {
    return '.';
  }
  const normalized = trimmed.replace(/\/+$/, '');
  return normalized || '.';
}

function ensureTrailingSlash(input) {
  if (!input) {
    return './';
  }
  return input.endsWith('/') ? input : `${input}/`;
}

function joinLibraryFile(root, fileName) {
  if (root === '/') {
    return `/${fileName}`;
  }
  if (root.endsWith('/')) {
    return `${root}${fileName}`;
  }
  return `${root}/${fileName}`;
}

function writeVirtualFile(module, filePath, buffer) {
  const pathPtr = allocCString(module, filePath);
  const dataPtr = allocBytes(module, buffer);
  const result = module._anisette_fs_write_file(pathPtr, dataPtr, buffer.length);
  module._free(pathPtr);
  module._free(dataPtr);
  if (result !== 0) {
    const message = readLastError(module);
    throw new Error(message || 'virtual fs write failed');
  }
}

function readBytes(module, ptr, len) {
  if (!ptr || !len) {
    return new Uint8Array();
  }
  return module.HEAPU8.slice(ptr, ptr + len);
}

function toBase64(bytes) {
  if (!bytes.length) {
    return '';
  }
  return Buffer.from(bytes).toString('base64');
}

function toAppleClientTime(date = new Date()) {
  return date.toISOString().replace(/\.\d{3}Z$/, 'Z');
}

function detectAppleLocale() {
  const locale = Intl.DateTimeFormat().resolvedOptions().locale || 'en-US';
  return locale.replace('-', '_');
}

async function fileExists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

const args = process.argv.slice(2);
const defaultStoreservicesPath = path.join(__dirname, 'arm64-v8a', 'libstoreservicescore.so');
const defaultCoreadiPath = path.join(__dirname, 'arm64-v8a', 'libCoreADI.so');

const storeservicesPath = args[0] ?? defaultStoreservicesPath;
const coreadiPath = args[1] ?? defaultCoreadiPath;
const libraryPath = args[2] ?? './anisette/';
const dsidRaw = args[3] ?? '-2';
let identifier = args[4] ?? '';
const traceWindowStartRaw = args[5] ?? '0';
const silent = process.env.ANISETTE_SILENT === '1';

if (!(await fileExists(storeservicesPath)) || !(await fileExists(coreadiPath))) {
  usage();
  process.exit(1);
}

const libraryRoot = normalizeLibraryRoot(libraryPath);
const libraryArg = ensureTrailingSlash(libraryRoot);
const resolvedLibraryPath = path.resolve(libraryRoot);
const devicePath = path.join(resolvedLibraryPath, 'device.json');
const adiPath = path.join(resolvedLibraryPath, 'adi.pb');
let deviceData = null;

if (await fileExists(devicePath)) {
  try {
    deviceData = JSON.parse(await fs.readFile(devicePath, 'utf8'));
  } catch {}
}

if (!identifier && deviceData) {
  try {
    if (deviceData && typeof deviceData.identifier === 'string' && deviceData.identifier) {
      identifier = deviceData.identifier;
    }
  } catch {}
}

const moduleFactory = (await import(pathToFileURL(modulePath).href)).default;
const module = await moduleFactory({
  locateFile(file) {
    if (file.endsWith('.wasm')) {
      return wasmPath;
    }
    return file;
  }
});

const storeservices = await fs.readFile(storeservicesPath);
const coreadi = await fs.readFile(coreadiPath);
if (await fileExists(adiPath)) {
  const adiData = await fs.readFile(adiPath);
  try {
    writeVirtualFile(module, joinLibraryFile(libraryRoot, 'adi.pb'), adiData);
  } catch (err) {
    console.error('anisette_fs_write_file failed:', err.message || err);
    process.exit(1);
  }
}
if (await fileExists(devicePath)) {
  const deviceData = await fs.readFile(devicePath);
  try {
    writeVirtualFile(module, joinLibraryFile(libraryRoot, 'device.json'), deviceData);
  } catch (err) {
    console.error('anisette_fs_write_file failed:', err.message || err);
    process.exit(1);
  }
}

const storeservicesPtr = allocBytes(module, storeservices);
const coreadiPtr = allocBytes(module, coreadi);
const libraryPtr = allocCString(module, libraryArg);
const provisioningPtr = allocCString(module, libraryArg);
const identifierPtr = allocCString(module, identifier);

const initResult = module._anisette_init_from_blobs(
  storeservicesPtr,
  storeservices.length,
  coreadiPtr,
  coreadi.length,
  libraryPtr,
  provisioningPtr,
  identifierPtr
);

module._free(storeservicesPtr);
module._free(coreadiPtr);
if (libraryPtr) {
  module._free(libraryPtr);
}
if (provisioningPtr) {
  module._free(provisioningPtr);
}
if (identifierPtr) {
  module._free(identifierPtr);
}

if (initResult !== 0) {
  console.error('anisette_init_from_blobs failed:', readLastError(module));
  process.exit(1);
}

const traceWindowStart = BigInt(traceWindowStartRaw);
if (traceWindowStart > 0n && typeof module._anisette_set_trace_window_start === 'function') {
  const traceResult = module._anisette_set_trace_window_start(traceWindowStart);
  if (traceResult !== 0) {
    console.error('anisette_set_trace_window_start failed:', readLastError(module));
    process.exit(1);
  }
}

const dsid = BigInt(dsidRaw);
const provisioned = module._anisette_is_machine_provisioned(dsid);
if (provisioned < 0) {
  console.error('anisette_is_machine_provisioned failed:', readLastError(module));
  process.exit(1);
}

if (provisioned !== 1 && !silent) {
  console.warn('device not provisioned, request_otp may fail');
}

const otpResult = module._anisette_request_otp(dsid);
if (otpResult !== 0) {
  console.error('anisette_request_otp failed:', readLastError(module));
  process.exit(1);
}

const otpBytes = readBytes(module, module._anisette_get_otp_ptr(), module._anisette_get_otp_len());
const midBytes = readBytes(module, module._anisette_get_mid_ptr(), module._anisette_get_mid_len());
const localUserUuid = (deviceData && typeof deviceData.localUUID === 'string') ? deviceData.localUUID : '';
const mdLu = process.env.ANISETTE_MD_LU_BASE64 === '1'
  ? Buffer.from(localUserUuid, 'utf8').toString('base64')
  : localUserUuid;
const headers = {
  'X-Apple-I-Client-Time': toAppleClientTime(),
  'X-Apple-I-MD': toBase64(otpBytes),
  'X-Apple-I-MD-LU': mdLu,
  'X-Apple-I-MD-M': toBase64(midBytes),
  'X-Apple-I-MD-RINFO': process.env.ANISETTE_MD_RINFO ?? '17106176',
  'X-Apple-I-SRL-NO': process.env.ANISETTE_SRL_NO ?? '0',
  'X-Apple-I-TimeZone': process.env.ANISETTE_TIMEZONE ?? 'UTC',
  'X-Apple-Locale': process.env.ANISETTE_LOCALE ?? detectAppleLocale(),
  'X-MMe-Client-Info': (deviceData && typeof deviceData.clientInfo === 'string') ? deviceData.clientInfo : '',
  'X-Mme-Device-Id': (deviceData && typeof deviceData.UUID === 'string') ? deviceData.UUID : ''
};

if (!silent) {
  console.log(JSON.stringify(headers, null, 2));
}
