// Main Anisette class — the public-facing API

import type { AnisetteDeviceConfig, AnisetteHeaders, InitOptions } from "./types.js";
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
} from "./utils.js";
import type { DeviceJson } from "./types.js";

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
  private libs: LibraryStore;
  private provisioning: ProvisioningSession;
  private dsid: bigint;
  private libraryPath: string;

  private constructor(
    bridge: WasmBridge,
    device: Device,
    libs: LibraryStore,
    provisioning: ProvisioningSession,
    dsid: bigint,
    libraryPath: string
  ) {
    this.bridge = bridge;
    this.device = device;
    this.libs = libs;
    this.provisioning = provisioning;
    this.dsid = dsid;
    this.libraryPath = libraryPath;
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
    const libraryPath = initOpts.libraryPath ?? DEFAULT_LIBRARY_PATH;
    const provisioningPath = initOpts.provisioningPath ?? libraryPath;
    const dsid = options.dsid ?? DEFAULT_DSID;

    // Load or generate device config
    const device = Device.fromJson(null, initOpts.deviceConfig);

    // Write device.json into WASM VFS so the emulator can read it
    const deviceJson = device.toJson();
    const deviceJsonBytes = encodeUtf8(JSON.stringify(deviceJson, null, 2));
    bridge.writeVirtualFile(joinPath(libraryPath, "device.json"), deviceJsonBytes);

    // Initialize WASM ADI
    bridge.initFromBlobs(
      libs.storeservicescore,
      libs.coreadi,
      libraryPath,
      provisioningPath,
      initOpts.identifier ?? device.adiIdentifier
    );

    const provisioning = new ProvisioningSession(
      bridge,
      device,
      options.httpClient
    );

    return new Anisette(bridge, device, libs, provisioning, dsid, libraryPath);
  }

  /**
   * Load a previously saved session (device.json + adi.pb written back into VFS).
   * Pass the saved device.json and adi.pb bytes alongside the library blobs.
   */
  static async fromSaved(
    storeservicescore: Uint8Array,
    coreadi: Uint8Array,
    deviceJsonBytes: Uint8Array,
    adiPbBytes: Uint8Array,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    wasmModule: any,
    options: AnisetteOptions = {}
  ): Promise<Anisette> {
    const bridge = new WasmBridge(wasmModule);
    const initOpts = options.init ?? {};
    const libraryPath = initOpts.libraryPath ?? DEFAULT_LIBRARY_PATH;
    const provisioningPath = initOpts.provisioningPath ?? libraryPath;
    const dsid = options.dsid ?? DEFAULT_DSID;

    // Parse saved device config
    let deviceJson: DeviceJson | null = null;
    try {
      deviceJson = JSON.parse(new TextDecoder().decode(deviceJsonBytes)) as DeviceJson;
    } catch {
      // ignore parse errors — will generate fresh device
    }
    const device = Device.fromJson(deviceJson, initOpts.deviceConfig);

    // Restore VFS files
    bridge.writeVirtualFile(joinPath(libraryPath, "device.json"), deviceJsonBytes);
    bridge.writeVirtualFile(joinPath(libraryPath, "adi.pb"), adiPbBytes);

    const libs = LibraryStore.fromBlobs(storeservicescore, coreadi);

    bridge.initFromBlobs(
      libs.storeservicescore,
      libs.coreadi,
      libraryPath,
      provisioningPath,
      initOpts.identifier ?? device.adiIdentifier
    );

    const provisioning = new ProvisioningSession(
      bridge,
      device,
      options.httpClient
    );

    return new Anisette(bridge, device, libs, provisioning, dsid, libraryPath);
  }

  // ---- public API ----

  /** Whether the device is currently provisioned. */
  get isProvisioned(): boolean {
    return this.bridge.isMachineProvisioned(this.dsid);
  }

  /** Run the provisioning flow against Apple servers. */
  async provision(): Promise<void> {
    await this.provisioning.provision(this.dsid);
  }

  /** Generate Anisette headers. Throws if not provisioned. */
  async getData(): Promise<AnisetteHeaders> {
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
