// Core type definitions for the Anisette JS/TS API

export interface AnisetteDeviceConfig {
  /** Human-readable device description sent to Apple servers */
  serverFriendlyDescription: string;
  /** Unique device UUID (uppercase) */
  uniqueDeviceId: string;
  /** ADI identifier (hex string) */
  adiId: string;
  /** Local user UUID (uppercase hex) */
  localUserUuid: string;
}

export interface AnisetteHeaders {
  "X-Apple-I-Client-Time": string;
  "X-Apple-I-MD": string;
  "X-Apple-I-MD-LU": string;
  "X-Apple-I-MD-M": string;
  "X-Apple-I-MD-RINFO": string;
  "X-Apple-I-SRL-NO": string;
  "X-Apple-I-TimeZone": string;
  "X-Apple-Locale": string;
  "X-MMe-Client-Info": string;
  "X-Mme-Device-Id": string;
}

export interface InitOptions {
  /** Path prefix used inside the WASM virtual filesystem for library files */
  libraryPath?: string;
  /** Path prefix used inside the WASM virtual filesystem for provisioning data */
  provisioningPath?: string;
  /** ADI identifier override */
  identifier?: string;
  /** Override parts of the generated device config */
  deviceConfig?: Partial<AnisetteDeviceConfig>;
}

/** Raw device.json structure as stored on disk / in WASM VFS */
export interface DeviceJson {
  UUID: string;
  clientInfo: string;
  identifier: string;
  localUUID: string;
}
