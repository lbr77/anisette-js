

// Set up Module configuration before loading libcurl
if (typeof window !== 'undefined') {
  window.Module = window.Module || {};
  window.Module.preRun = window.Module.preRun || [];
  window.Module.preRun.push(function() {
    // Get the default CA certs from libcurl (will be available after WASM loads)
    // We'll create the extended CA file here
    console.log('[preRun] Setting up extended CA certificates');
  });
}

import { libcurl } from './libcurl.mjs';

const DEFAULT_CONFIG = {
  glueUrl: './anisette/anisette_rs.js',
  storeservicesUrl: './arm64-v8a/libstoreservicescore.so',
  coreadiUrl: './arm64-v8a/libCoreADI.so',
  libraryPath: './anisette/',
  provisioningPath: './anisette/',
  identifier: '',
  dsid: '-2',
  assetVersion: '',
  rustBacktrace: 'full',
  rustLibBacktrace: '1',
};

const state = {
  module: null,
  storeservicesBytes: null,
  coreadiBytes: null,
};

const CONFIG = loadConfig();
const logEl = document.getElementById('log');

function log(message) {
  const line = `${message}`;
  console.log(line);
  logEl.textContent += `${line}\n`;
}

function loadConfig() {
  const cfg = { ...DEFAULT_CONFIG };
  const params = new URLSearchParams(window.location.search);
  for (const key of Object.keys(cfg)) {
    const value = params.get(key);
    if (value !== null) {
      cfg[key] = value;
    }
  }
  if (!cfg.assetVersion) {
    cfg.assetVersion = String(Date.now());
  }
  return cfg;
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function normalizeMountPath(path) {
  const trimmed = (path || '').trim();
  if (!trimmed || trimmed === '/' || trimmed === './' || trimmed === '.') {
    return '/';
  }

  const noTrailing = trimmed.replace(/\/+$/, '');
  const noDot = noTrailing.startsWith('./') ? noTrailing.slice(1) : noTrailing;
  if (noDot.startsWith('/')) {
    return noDot;
  }
  return `/${noDot}`;
}

function bytesToBase64(bytes) {
  let s = '';
  for (let i = 0; i < bytes.length; i += 1) {
    s += String.fromCharCode(bytes[i]);
  }
  return btoa(s);
}

function base64ToBytes(text) {
  const clean = (text || '').trim();
  if (!clean) {
    return new Uint8Array();
  }
  const s = atob(clean);
  const out = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i += 1) {
    out[i] = s.charCodeAt(i);
  }
  return out;
}

function dsidToU64(value) {
  return BigInt.asUintN(64, BigInt(value.trim()));
}

async function fetchBytes(url, label) {
  const res = await fetch(url);
  assert(res.ok, `${label} fetch failed: HTTP ${res.status} (${url})`);
  return new Uint8Array(await res.arrayBuffer());
}

function ensureExport(name) {
  const fn = state.module[name];
  if (typeof fn !== 'function') {
    throw new Error(`missing export ${name}`);
  }
  return fn;
}

function allocBytes(bytes) {
  if (bytes.length === 0) {
    return 0;
  }
  const malloc = ensureExport('_malloc');
  const ptr = Number(malloc(bytes.length));
  state.module.HEAPU8.set(bytes, ptr);
  return ptr;
}

function allocCString(text) {
  const value = text || '';
  const size = state.module.lengthBytesUTF8(value) + 1;
  const malloc = ensureExport('_malloc');
  const ptr = Number(malloc(size));
  state.module.stringToUTF8(value, ptr, size);
  return ptr;
}

function readBytes(ptr, len) {
  return state.module.HEAPU8.slice(Number(ptr), Number(ptr) + Number(len));
}

function readRustError() {
  const getPtr = ensureExport('_anisette_last_error_ptr');
  const getLen = ensureExport('_anisette_last_error_len');
  const ptr = Number(getPtr());
  const len = Number(getLen());
  if (len === 0) {
    return '';
  }
  return new TextDecoder().decode(readBytes(ptr, len));
}

function call(name, fn) {
  let ret;
  try {
    ret = fn();
  } catch (e) {
    log(`${name}: trap=${String(e)}`);
    throw e;
  }
  const err = readRustError();
  log(`${name}: ret=${ret}${err ? ` err=${err}` : ''}`);
  if (ret < 0) {
    throw new Error(`${name} failed: ${err || `ret=${ret}`}`);
  }
  return { ret, err };
}

function resolveWasmUrl(jsUrl) {
  const url = new URL(jsUrl, window.location.origin);
  if (!url.pathname.endsWith('.js')) {
    throw new Error(`invalid glue path (expect .js): ${url.href}`);
  }
  url.pathname = url.pathname.slice(0, -3) + '.wasm';
  return url.href;
}

async function initModule() {
  log(`config: ${JSON.stringify(CONFIG)}`);

  state.storeservicesBytes = await fetchBytes(CONFIG.storeservicesUrl, 'libstoreservicescore.so');
  state.coreadiBytes = await fetchBytes(CONFIG.coreadiUrl, 'libCoreADI.so');

  const moduleUrl = new URL(CONFIG.glueUrl, window.location.origin);
  moduleUrl.searchParams.set('v', CONFIG.assetVersion);
  const createModule = (await import(moduleUrl.href)).default;
  const wasmUrl = resolveWasmUrl(moduleUrl.href);
  log(`glue_url=${moduleUrl.href}`);
  log(`wasm_url=${wasmUrl}`);

  const wasmRes = await fetch(wasmUrl, { cache: 'no-store' });
  assert(wasmRes.ok, `wasm fetch failed: HTTP ${wasmRes.status} (${wasmUrl})`);
  const wasmBinary = new Uint8Array(await wasmRes.arrayBuffer());
  assert(wasmBinary.length >= 8, `wasm too small: ${wasmBinary.length} bytes`);
  const magicOk =
    wasmBinary[0] === 0x00 &&
    wasmBinary[1] === 0x61 &&
    wasmBinary[2] === 0x73 &&
    wasmBinary[3] === 0x6d;
  if (!magicOk) {
    const head = Array.from(wasmBinary.slice(0, 8))
      .map((b) => b.toString(16).padStart(2, '0'))
      .join(' ');
    throw new Error(`invalid wasm magic at ${wasmUrl}, first8=${head}`);
  }

  state.module = await createModule({
    noInitialRun: true,
    wasmBinary,
    ENV: {
      RUST_BACKTRACE: CONFIG.rustBacktrace,
      RUST_LIB_BACKTRACE: CONFIG.rustLibBacktrace,
    },
    print: (msg) => log(`${msg}`),
    printErr: (msg) => log(`${msg}`),
    locateFile: (path) => {
      if (path.includes('.wasm')) {
        return wasmUrl;
      }
      return path;
    },
  });

  log('emscripten module instantiated');
}

async function syncIdbfs(populate) {
  const FS = state.module.FS;
  await new Promise((resolve, reject) => {
    FS.syncfs(populate, (err) => {
      if (err) {
        reject(err);
      } else {
        resolve();
      }
    });
  });
}

async function initIdbfs() {
  const FS = state.module.FS;
  const IDBFS = FS.filesystems?.IDBFS;
  const mountPath = normalizeMountPath(CONFIG.libraryPath);

  if (!IDBFS) {
    throw new Error('IDBFS unavailable on FS.filesystems');
  }

  if (mountPath !== '/') {
    try {
      FS.mkdirTree(mountPath);
    } catch (_) {
      // ignore existing path
    }
  }

  try {
    FS.mount(IDBFS, {}, mountPath);
  } catch (_) {
    // ignore already mounted
  }

  await syncIdbfs(true);
  log(`idbfs mounted: ${mountPath}`);
}

async function persistIdbfs() {
  await syncIdbfs(false);
  log('idbfs sync: flushed');
}

// ===== HTTP Request helpers using libcurl =====

const USER_AGENT = 'akd/1.0 CFNetwork/1404.0.5 Darwin/22.3.0';

async function httpGet(url, extraHeaders = {}) {
  log(`GET ${url}`);
  const headers = {
    'User-Agent': USER_AGENT,
    ...extraHeaders,
  };
  const resp = await libcurl.fetch(url, {
    method: 'GET',
    headers,
    redirect: 'manual',
    _libcurl_http_version: 1.1,
    insecure: true,
  });
  const body = await resp.text();
  log(`GET ${url} -> ${resp.status}`);
  return { status: resp.status, body };
}

async function httpPost(url, data, extraHeaders = {}) {
  log(`POST ${url}`);
  const headers = {
    'User-Agent': USER_AGENT,
    'Content-Type': 'application/x-www-form-urlencoded',
    'Connection': 'keep-alive',
    ...extraHeaders,
  };
  const resp = await libcurl.fetch(url, {
    method: 'POST',
    headers,
    body: data,
    redirect: 'manual',
    insecure: true,
    _libcurl_http_version: 1.1,
  });
  const body = await resp.text();
  log(`POST ${url} -> ${resp.status}`);
  return { status: resp.status, body };
}

// Simple plist parsing for the specific format we need
function parsePlist(xmlText) {
  const parser = new DOMParser();
  const doc = parser.parseFromString(xmlText, 'text/xml');
  
  function parseNode(node) {
    if (!node) return null;
    
    const tag = node.tagName;
    if (tag === 'dict') {
      const result = {};
      let key = null;
      for (const child of node.children) {
        if (child.tagName === 'key') {
          key = child.textContent;
        } else if (key !== null) {
          result[key] = parseNode(child);
          key = null;
        }
      }
      return result;
    } else if (tag === 'array') {
      return Array.from(node.children).map(parseNode);
    } else if (tag === 'string') {
      return node.textContent || '';
    } else if (tag === 'integer') {
      return parseInt(node.textContent, 10);
    } else if (tag === 'true') {
      return true;
    } else if (tag === 'false') {
      return false;
    } else if (tag === 'data') {
      // base64 encoded data
      const text = node.textContent || '';
      return text.replace(/\s/g, '');
    }
    return null;
  }
  
  const plist = doc.querySelector('plist > dict, plist > array');
  return parseNode(plist);
}

// ===== Device Management =====

class Device {
  constructor() {
    this.uniqueDeviceIdentifier = '';
    this.serverFriendlyDescription = '';
    this.adiIdentifier = '';
    this.localUserUuid = '';
    this.initialized = false;
  }

  generate() {
    // Generate UUID
    this.uniqueDeviceIdentifier = crypto.randomUUID().toUpperCase();
    
    // Pretend to be a MacBook Pro like in index.py
    this.serverFriendlyDescription = '<MacBookPro13,2> <macOS;13.1;22C65> <com.apple.AuthKit/1 (com.apple.dt.Xcode/3594.4.19)>';
    
    // Generate 16 hex chars (8 bytes) for ADI identifier
    const bytes = new Uint8Array(8);
    crypto.getRandomValues(bytes);
    this.adiIdentifier = Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');
    
    // Generate 64 hex chars (32 bytes) for local user UUID
    const luBytes = new Uint8Array(32);
    crypto.getRandomValues(luBytes);
    this.localUserUuid = Array.from(luBytes, b => b.toString(16).toUpperCase().padStart(2, '0')).join('');
    
    this.initialized = true;
    log(`Device generated: UDID=${this.uniqueDeviceIdentifier}, ADI=${this.adiIdentifier}`);
  }
}

function deviceFilePath() {
  const mountPath = normalizeMountPath(CONFIG.libraryPath);
  if (mountPath === '/') {
    return '/device.json';
  }
  return `${mountPath}/device.json`;
}

function parseDeviceRecord(record) {
  if (!record || typeof record !== 'object') {
    return null;
  }
  const device = new Device();
  device.uniqueDeviceIdentifier = String(record.UUID || '');
  device.serverFriendlyDescription = String(record.clientInfo || '');
  device.adiIdentifier = String(record.identifier || '');
  device.localUserUuid = String(record.localUUID || '');
  device.initialized = Boolean(
    device.uniqueDeviceIdentifier &&
    device.serverFriendlyDescription &&
    device.adiIdentifier &&
    device.localUserUuid,
  );
  return device;
}

function serializeDevice(device) {
  return {
    UUID: device.uniqueDeviceIdentifier,
    clientInfo: device.serverFriendlyDescription,
    identifier: device.adiIdentifier,
    localUUID: device.localUserUuid,
  };
}

function readDeviceFromFs() {
  const FS = state.module.FS;
  const path = deviceFilePath();
  try {
    const text = FS.readFile(path, { encoding: 'utf8' });
    const parsed = JSON.parse(text);
    const device = parseDeviceRecord(parsed);
    if (!device || !device.initialized) {
      return null;
    }
    log(`Device loaded: UDID=${device.uniqueDeviceIdentifier}, ADI=${device.adiIdentifier}`);
    return device;
  } catch (e) {
    return null;
  }
}

function persistDevice(device) {
  const FS = state.module.FS;
  const path = deviceFilePath();
  const text = JSON.stringify(serializeDevice(device), null, 2);
  FS.writeFile(path, text);
  log(`Device persisted: ${path}`);
}

function loadOrCreateDevice() {
  let device = readDeviceFromFs();
  let shouldPersist = false;

  if (!device) {
    device = new Device();
    device.generate();
    shouldPersist = true;
  }

  const override = (CONFIG.identifier || '').trim();
  if (override && override !== device.adiIdentifier) {
    device.adiIdentifier = override;
    device.initialized = true;
    shouldPersist = true;
    log(`Device identifier overridden: ${override}`);
  }

  if (shouldPersist) {
    persistDevice(device);
  }

  return device;
}

// ===== Provisioning Session =====

class ProvisioningSession {
  constructor(device) {
    this.device = device;
    this.urlBag = {};
  }

  getBaseHeaders() {
    return {
      'X-Mme-Device-Id': this.device.uniqueDeviceIdentifier,
      'X-MMe-Client-Info': this.device.serverFriendlyDescription,
      'X-Apple-I-MD-LU': this.device.localUserUuid,
      'X-Apple-Client-App-Name': 'Setup',
    };
  }

  getClientTime() {
    // ISO format without milliseconds, like Python's isoformat()
    return new Date().toISOString().replace(/\.\d{3}Z$/, 'Z');
  }

  async loadUrlBag() {
    if (Object.keys(this.urlBag).length > 0) {
      return;
    }
    
    const url = 'https://gsa.apple.com/grandslam/GsService2/lookup';
    const { body } = await httpGet(url, this.getBaseHeaders());
    
    const plist = parsePlist(body);
    if (plist && plist.urls) {
      this.urlBag = plist.urls;
      log(`URL bag loaded: ${Object.keys(this.urlBag).join(', ')}`);
    } else {
      throw new Error('Failed to parse URL bag');
    }
  }

  async provision(dsId) {
    log('Starting provisioning...');
    
    await this.loadUrlBag();

    // Step 1: Start provisioning - get spim from Apple
    const startProvisioningPlist = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Header</key>
	<dict/>
	<key>Request</key>
	<dict/>
</dict>
</plist>`;

    const extraHeadersStart = {
      ...this.getBaseHeaders(),
      'X-Apple-I-Client-Time': this.getClientTime(),
    };

    const { body: startBody } = await httpPost(
      this.urlBag.midStartProvisioning,
      startProvisioningPlist,
      extraHeadersStart
    );

    const spimPlist = parsePlist(startBody);
    const spimStr = spimPlist?.Response?.spim;
    if (!spimStr) {
      throw new Error('Failed to get spim from start provisioning');
    }
    
    const spim = base64ToBytes(spimStr);
    log(`Got spim: ${spim.length} bytes`);

    // Step 2: Call ADI start_provisioning
    const cpim = await this.adiStartProvisioning(dsId, spim);
    log(`Got cpim: ${cpim.length} bytes`);

    // Step 3: End provisioning - send cpim to Apple
    const endProvisioningPlist = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Header</key>
	<dict/>
	<key>Request</key>
	<dict>
		<key>cpim</key>
		<string>${bytesToBase64(cpim)}</string>
	</dict>
</dict>
</plist>`;

    const extraHeadersEnd = {
      ...this.getBaseHeaders(),
      'X-Apple-I-Client-Time': this.getClientTime(),
    };

    const { body: endBody } = await httpPost(
      this.urlBag.midFinishProvisioning,
      endProvisioningPlist,
      extraHeadersEnd
    );

    const endPlist = parsePlist(endBody);
    const response = endPlist?.Response;
    if (!response) {
      throw new Error('Failed to get response from end provisioning');
    }

    const ptm = base64ToBytes(response.ptm);
    const tk = base64ToBytes(response.tk);
    log(`Got ptm: ${ptm.length} bytes, tk: ${tk.length} bytes`);

    // Step 4: Call ADI end_provisioning
    await this.adiEndProvisioning(ptm, tk);
    log('Provisioning completed successfully');
  }

  async adiStartProvisioning(dsId, spim) {
    const pSpim = allocBytes(spim);
    const startFn = ensureExport('_anisette_start_provisioning');
    const start = call('anisette_start_provisioning', () =>
      Number(startFn(dsId, pSpim, spim.length)),
    );

    if (start.ret !== 0) {
      throw new Error(`start_provisioning failed: ${start.err || 'unknown error'}`);
    }

    const getCpimPtr = ensureExport('_anisette_get_cpim_ptr');
    const getCpimLen = ensureExport('_anisette_get_cpim_len');
    const getSession = ensureExport('_anisette_get_session');
    
    const cpimPtr = Number(getCpimPtr());
    const cpimLen = Number(getCpimLen());
    state.session = Number(getSession());
    
    return readBytes(cpimPtr, cpimLen);
  }

  async adiEndProvisioning(ptm, tk) {
    const pPtm = allocBytes(ptm);
    const pTk = allocBytes(tk);
    const endFn = ensureExport('_anisette_end_provisioning');
    const end = call('anisette_end_provisioning', () =>
      Number(endFn(state.session, pPtm, ptm.length, pTk, tk.length)),
    );

    if (end.ret !== 0) {
      throw new Error(`end_provisioning failed: ${end.err || 'unknown error'}`);
    }
  }
}

// ===== Main Flow =====

async function initAnisette(identifier) {
  const pStores = allocBytes(state.storeservicesBytes);
  const pCore = allocBytes(state.coreadiBytes);
  const pLibrary = allocCString(CONFIG.libraryPath);
  const pProvisioning = allocCString(CONFIG.provisioningPath);
  const pIdentifier = allocCString(identifier);

  const initFromBlobs = ensureExport('_anisette_init_from_blobs');
  const init = call('anisette_init_from_blobs', () =>
    Number(
      initFromBlobs(
        pStores,
        state.storeservicesBytes.length,
        pCore,
        state.coreadiBytes.length,
        pLibrary,
        pProvisioning,
        pIdentifier,
      ),
    ),
  );
  if (init.ret !== 0) {
    throw new Error('init failed');
  }
}

async function isMachineProvisioned(dsId) {
  const isProvisionedFn = ensureExport('_anisette_is_machine_provisioned');
  const provisioned = call('anisette_is_machine_provisioned', () =>
    Number(isProvisionedFn(dsId)),
  );
  return provisioned.ret !== 0; // ret === 0 means provisioned (no error)
}

async function requestOtp(dsId) {
  const otpFn = ensureExport('_anisette_request_otp');
  const otp = call('anisette_request_otp', () => Number(otpFn(dsId)));
  if (otp.ret !== 0) {
    throw new Error(`request_otp failed: ${otp.err || 'unknown error'}`);
  }

  const getOtpPtr = ensureExport('_anisette_get_otp_ptr');
  const getOtpLen = ensureExport('_anisette_get_otp_len');
  const getMidPtr = ensureExport('_anisette_get_mid_ptr');
  const getMidLen = ensureExport('_anisette_get_mid_len');

  const otpPtr = Number(getOtpPtr());
  const otpLen = Number(getOtpLen());
  const midPtr = Number(getMidPtr());
  const midLen = Number(getMidLen());

  const otpBytes = readBytes(otpPtr, otpLen);
  const midBytes = readBytes(midPtr, midLen);

  return {
    oneTimePassword: otpBytes,
    machineIdentifier: midBytes,
  };
}

async function initLibcurl() {
  log('initializing libcurl...');
  const wsProto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  libcurl.set_websocket(`${wsProto}//${location.host}/wisp/`);
  
  // Capture libcurl verbose output
  libcurl.stderr = (text) => {
    log(`[libcurl] ${text}`);
  };
  
  await libcurl.load_wasm('./libcurl.wasm');
  
  // // Get default CA certs and append Apple CA certs
  // const defaultCacert = libcurl.get_cacert();
  // const extendedCacert = defaultCacert + '\n' + APPLE_CA_CERTS;
  
  // Create a file with extended CA certs in the Emscripten FS
  
  log('libcurl initialized');
}


  function dumpFs(path = '/') {
    const FS = state.module.FS;
    const entries = FS.readdir(path).filter((name) => name !== '.' && name !== '..');
    for (const name of entries) {
      const full = path === '/' ? `/${name}` : `${path}/${name}`;
      const stat = FS.stat(full);
      if (FS.isDir(stat.mode)) {
        console.log(`dir  ${full}`);
        dumpFs(full);
      } else {
        const data = FS.readFile(full);
        console.log(`file ${full} size=${data.length}`);
        // 如果要看内容（文本）
        // console.log(new TextDecoder().decode(data));
        // base64
        function bytesToHex(bytes) {
          return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join(' ');
        }
        console.log(bytesToBase64(data))
      }
    }
  }

async function main() {
  try {
    // Initialize libcurl for HTTP requests
    await initLibcurl();

    // Load WASM module
    await initModule();

    // Initialize IDBFS
    await initIdbfs();

    // Load device info or generate new one
    const device = loadOrCreateDevice();

    // Initialize anisette with device identifier
    await initAnisette(device.adiIdentifier);

    const dsid = dsidToU64(CONFIG.dsid);

    // Check if machine is provisioned
    const isProvisioned = await isMachineProvisioned(dsid);
    
    if (!isProvisioned) {
      log('Machine not provisioned, starting provisioning...');
      const session = new ProvisioningSession(device);
      await session.provision(dsid);
    } else {
      log('Machine already provisioned');
    }

    // Request OTP
    log('Requesting OTP...');
    const otp = await requestOtp(dsid);

    // Output the result
    const result = {
      'X-Apple-I-MD': bytesToBase64(otp.oneTimePassword),
      'X-Apple-I-MD-M': bytesToBase64(otp.machineIdentifier),
    };
    log(`OTP result: ${JSON.stringify(result, null, 2)}`);

    // Persist IDBFS
    await persistIdbfs();
    // dumpFs("/anisette/");
    log('done');
  } catch (e) {
    log(`fatal: ${String(e)}`);
    console.error(e);
  }
}

main();
