// Main Anisette class — the public-facing API

import type { AnisetteHeaders, InitOptions } from "./types.js";
import type { HttpClient } from "./http.js";
import { WasmBridge } from "./wasm-bridge.js";
import { Device } from "./device.js";
import { LibraryStore } from "./library.js";
import { ProvisioningSession } from "./provisioning.js";
import {
  toBase64,
  toAppleClientTime,
  detectLocale,
  encodeUtf8,
  decodeUtf8,
} from "./utils.js";

const DEFAULT_DSID = BigInt(-2);
const DEFAULT_LIBRARY_PATH = "./anisette/";
const MD_RINFO = "17106176";

export interface AnisetteOptions {
  /** Override the HTTP client (useful for testing or custom proxy) */
  httpClient?: HttpClient;
  /** DSID to use when requesting OTP (default: -2) */
  dsid?: bigint;
  /** Options passed to WASM init */
  init?: InitOptions;
}

export class Anisette {
  private bridge: WasmBridge;
  private device: Device;
  private provisioning: ProvisioningSession;
  private dsid: bigint;
  private provisioningPath: string;
  private libraryPath: string;
  private libs: LibraryStore;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private wasmModule: any;
  private identifier: string;
  private httpClient: HttpClient | undefined;

  private constructor(
    bridge: WasmBridge,
    device: Device,
    provisioning: ProvisioningSession,
    dsid: bigint,
    provisioningPath: string,
    libraryPath: string,
    libs: LibraryStore,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    wasmModule: any,
    identifier: string,
    httpClient: HttpClient | undefined,
  ) {
    this.bridge = bridge;
    this.device = device;
    this.provisioning = provisioning;
    this.dsid = dsid;
    this.provisioningPath = provisioningPath;
    this.libraryPath = libraryPath;
    this.libs = libs;
    this.wasmModule = wasmModule;
    this.identifier = identifier;
    this.httpClient = httpClient;
  }

  // ---- factory methods ----

  /**
   * Initialize from the two Android .so library files.
   * @param storeservicescore - bytes of libstoreservicescore.so
   * @param coreadi           - bytes of libCoreADI.so
   */
  static async fromSo(
    storeservicescore: Uint8Array,
    coreadi: Uint8Array,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    wasmModule: any,
    options: AnisetteOptions = {}
  ): Promise<Anisette> {
    const libs = LibraryStore.fromBlobs(storeservicescore, coreadi);
    return Anisette._init(libs, wasmModule, options);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private static async _init(
    libs: LibraryStore,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    wasmModule: any,
    options: AnisetteOptions
  ): Promise<Anisette> {
    const bridge = new WasmBridge(wasmModule);
    const initOpts = options.init ?? {};
    const libraryPath = normalizeAdiPath(initOpts.libraryPath ?? DEFAULT_LIBRARY_PATH);
    const provisioningPath = normalizeAdiPath(initOpts.provisioningPath ?? libraryPath);
    const dsid = options.dsid ?? DEFAULT_DSID;

    // Mount + load persisted IDBFS first so file state is stable before init.
    mountIdbfsPaths(bridge, libraryPath, provisioningPath);
    try {
      await bridge.syncIdbfsFromStorage();
    } catch {
      // Ignore errors - might be first run with no existing data
    }

    // Load device config from explicit bytes first, then from persisted VFS.
    const savedDeviceJson =
      parseDeviceJsonBytes(initOpts.deviceJsonBytes) ??
      readDeviceJsonFromVfs(bridge, joinPath(libraryPath, "device.json"));
    const device = Device.fromJson(savedDeviceJson, initOpts.deviceConfig);
    const identifier = initOpts.identifier ?? device.adiIdentifier;

    // Restore explicit adi.pb into VFS if provided.
    if (initOpts.adiPb) {
      bridge.writeVirtualFile(joinPath(provisioningPath, "adi.pb"), initOpts.adiPb);
    }

    // Keep VFS device.json consistent with the active in-memory device.
    const deviceJsonBytes = initOpts.deviceJsonBytes ?? encodeUtf8(JSON.stringify(device.toJson(), null, 2));
    bridge.writeVirtualFile(joinPath(libraryPath, "device.json"), deviceJsonBytes);

    // Initialize WASM ADI
    bridge.initFromBlobs(
      libs.storeservicescore,
      libs.coreadi,
      libraryPath,
      provisioningPath,
      identifier
    );

    const provisioning = new ProvisioningSession(
      bridge,
      device,
      options.httpClient
    );

    return new Anisette(bridge, device, provisioning, dsid, provisioningPath, libraryPath, libs, wasmModule, identifier, options.httpClient);
  }

  // ---- public API ----

  /** Whether the device is currently provisioned. */
  get isProvisioned(): boolean {
    return this.bridge.isMachineProvisioned(this.dsid);
  }

  /** Run the provisioning flow against Apple servers. */
  async provision(): Promise<void> {
    await this.provisioning.provision(this.dsid);
    // Sync provisioning state to IndexedDB (browser only)
    try {
      await this.bridge.syncIdbfsToStorage();
    } catch {
      // Ignore errors in Node.js or if IDBFS unavailable
    }
  }

  /** Read adi.pb from the WASM VFS for persistence. */
  getAdiPb(): Uint8Array {
    return this.bridge.readVirtualFile(joinPath(this.provisioningPath, "adi.pb"));
  }

  /** Generate Anisette headers. Throws if not provisioned. */
  async getData(): Promise<AnisetteHeaders> {
    // Reinit WASM state before each call to avoid emulator corruption on repeated use
    const adiPb = readOptionalFile(
      this.bridge,
      joinPath(this.provisioningPath, "adi.pb")
    );
    const deviceJsonBytes = encodeUtf8(JSON.stringify(this.device.toJson(), null, 2));

    this.bridge = new WasmBridge(this.wasmModule);
    mountIdbfsPaths(this.bridge, this.libraryPath, this.provisioningPath);
    try {
      await this.bridge.syncIdbfsFromStorage();
    } catch {
      // Ignore errors - might be first run or Node.js
    }

    if (adiPb) {
      this.bridge.writeVirtualFile(joinPath(this.provisioningPath, "adi.pb"), adiPb);
    }
    this.bridge.writeVirtualFile(joinPath(this.libraryPath, "device.json"), deviceJsonBytes);

    this.bridge.initFromBlobs(this.libs.storeservicescore, this.libs.coreadi, this.libraryPath, this.provisioningPath, this.identifier);

    this.provisioning = new ProvisioningSession(this.bridge, this.device, this.httpClient);

    const { otp, machineId } = this.bridge.requestOtp(this.dsid);

    const now = new Date();
    const tzOffset = -now.getTimezoneOffset();
    const tzSign = tzOffset >= 0 ? "+" : "-";
    const tzHours = String(Math.floor(Math.abs(tzOffset) / 60)).padStart(2, "0");
    const tzMins = String(Math.abs(tzOffset) % 60).padStart(2, "0");
    const timezone = `${tzSign}${tzHours}${tzMins}`;

    return {
      "X-Apple-I-Client-Time": toAppleClientTime(now),
      "X-Apple-I-MD": toBase64(otp),
      "X-Apple-I-MD-LU": this.device.localUserUuid,
      "X-Apple-I-MD-M": toBase64(machineId),
      "X-Apple-I-MD-RINFO": MD_RINFO,
      "X-Apple-I-SRL-NO": "0",
      "X-Apple-I-TimeZone": timezone,
      "X-Apple-Locale": detectLocale(),
      "X-MMe-Client-Info": this.device.serverFriendlyDescription,
      "X-Mme-Device-Id": this.device.uniqueDeviceIdentifier,
    };
  }

  /** Serialize device.json bytes for persistence. */
  getDeviceJson(): Uint8Array {
    return encodeUtf8(JSON.stringify(this.device.toJson(), null, 2));
  }

  /** Expose the device for inspection. */
  getDevice(): Device {
    return this.device;
  }
}

function joinPath(base: string, file: string): string {
  const b = base.endsWith("/") ? base : `${base}/`;
  return `${b}${file}`;
}

function normalizeAdiPath(path: string): string {
  const trimmed = path.trim().replace(/\\/g, "/");
  if (!trimmed || trimmed === "." || trimmed === "./" || trimmed === "/") {
    return "./";
  }

  const noTrail = trimmed.replace(/\/+$/, "");
  if (!noTrail || noTrail === ".") {
    return "./";
  }

  if (noTrail.startsWith("./") || noTrail.startsWith("../")) {
    return `${noTrail}/`;
  }
  if (noTrail.startsWith("/")) {
    return `.${noTrail}/`;
  }
  return `./${noTrail}/`;
}

function mountIdbfsPaths(
  bridge: WasmBridge,
  libraryPath: string,
  provisioningPath: string
): void {
  const paths = new Set([libraryPath, provisioningPath]);
  for (const path of paths) {
    bridge.initIdbfs(path);
  }
}

function readOptionalFile(bridge: WasmBridge, path: string): Uint8Array | null {
  try {
    return bridge.readVirtualFile(path);
  } catch {
    return null;
  }
}

function parseDeviceJsonBytes(
  bytes: Uint8Array | undefined
): import("./types.js").DeviceJson | null {
  if (!bytes) {
    return null;
  }
  try {
    return JSON.parse(decodeUtf8(bytes)) as import("./types.js").DeviceJson;
  } catch {
    return null;
  }
}

function readDeviceJsonFromVfs(
  bridge: WasmBridge,
  path: string
): import("./types.js").DeviceJson | null {
  const bytes = readOptionalFile(bridge, path);
  if (!bytes) {
    return null;
  }
  return parseDeviceJsonBytes(bytes);
}
