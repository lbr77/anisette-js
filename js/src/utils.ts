// Utility functions shared across modules

const TEXT_ENCODER = new TextEncoder();
const TEXT_DECODER = new TextDecoder("utf-8");

/** Encode string to UTF-8 bytes */
export function encodeUtf8(str: string): Uint8Array {
  return TEXT_ENCODER.encode(str);
}

/** Decode UTF-8 bytes to string */
export function decodeUtf8(bytes: Uint8Array): string {
  return TEXT_DECODER.decode(bytes);
}

/** Encode bytes to base64 string */
export function toBase64(bytes: Uint8Array): string {
  if (bytes.length === 0) return "";
  // Works in both browser and Node.js (Node 16+)
  if (typeof Buffer !== "undefined") {
    return Buffer.from(bytes).toString("base64");
  }
  let binary = "";
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

/** Decode base64 string to bytes */
export function fromBase64(b64: string): Uint8Array {
  if (typeof Buffer !== "undefined") {
    return new Uint8Array(Buffer.from(b64, "base64"));
  }
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

/** Format a Date as Apple client time string (ISO 8601 without milliseconds) */
export function toAppleClientTime(date: Date = new Date()): string {
  return date.toISOString().replace(/\.\d{3}Z$/, "Z");
}

/** Detect locale string in Apple format (e.g. "en_US") */
export function detectLocale(): string {
  const locale =
    (typeof Intl !== "undefined" &&
      Intl.DateTimeFormat().resolvedOptions().locale) ||
    "en-US";
  return locale.replace("-", "_");
}

/** Generate a random hex string of the given byte length */
export function randomHex(byteLen: number, uppercase = false): string {
  const bytes = new Uint8Array(byteLen);
  if (typeof crypto !== "undefined" && crypto.getRandomValues) {
    crypto.getRandomValues(bytes);
  } else {
    // Node.js fallback
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const nodeCrypto = require("crypto") as typeof import("crypto");
    const buf = nodeCrypto.randomBytes(byteLen);
    bytes.set(buf);
  }
  let hex = Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
  return uppercase ? hex.toUpperCase() : hex;
}

/** Generate a random UUID v4 (uppercase) */
export function randomUUID(): string {
  if (typeof crypto !== "undefined" && crypto.randomUUID) {
    return crypto.randomUUID().toUpperCase();
  }
  // Manual fallback
  const hex = randomHex(16);
  return [
    hex.slice(0, 8),
    hex.slice(8, 12),
    "4" + hex.slice(13, 16),
    ((parseInt(hex[16], 16) & 0x3) | 0x8).toString(16) + hex.slice(17, 20),
    hex.slice(20, 32),
  ]
    .join("-")
    .toUpperCase();
}
