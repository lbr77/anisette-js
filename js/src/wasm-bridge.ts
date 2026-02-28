// Low-level bridge to the Emscripten-generated WASM module.
// Handles all pointer/length marshalling so higher layers never touch raw memory.

export interface StartProvisioningResult {
  cpim: Uint8Array;
  session: number;
}

export interface RequestOtpResult {
  otp: Uint8Array;
  machineId: Uint8Array;
}

export class WasmBridge {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private m: any;

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  constructor(wasmModule: any) {
    this.m = wasmModule;
  }

  // ---- memory helpers ----

  private allocBytes(bytes: Uint8Array): number {
    const ptr = this.m._malloc(bytes.length) as number;
    this.m.HEAPU8.set(bytes, ptr);
    return ptr;
  }

  private allocCString(value: string | null | undefined): number {
    if (!value) return 0;
    const size = (this.m.lengthBytesUTF8(value) as number) + 1;
    const ptr = this.m._malloc(size) as number;
    this.m.stringToUTF8(value, ptr, size);
    return ptr;
  }

  private readBytes(ptr: number, len: number): Uint8Array {
    if (!ptr || !len) return new Uint8Array(0);
    return (this.m.HEAPU8 as Uint8Array).slice(ptr, ptr + len);
  }

  private free(ptr: number): void {
    if (ptr) this.m._free(ptr);
  }

  // ---- error handling ----

  getLastError(): string {
    const ptr = this.m._anisette_last_error_ptr() as number;
    const len = this.m._anisette_last_error_len() as number;
    if (!ptr || !len) return "";
    const bytes = (this.m.HEAPU8 as Uint8Array).subarray(ptr, ptr + len);
    return new TextDecoder("utf-8").decode(bytes);
  }

  private check(result: number, context: string): void {
    if (result !== 0) {
      const msg = this.getLastError();
      throw new Error(`${context}: ${msg || "unknown error"}`);
    }
  }

  // ---- public API ----

  /**
   * Initialize ADI from in-memory library blobs.
   */
  initFromBlobs(
    storeservices: Uint8Array,
    coreadi: Uint8Array,
    libraryPath: string,
    provisioningPath?: string,
    identifier?: string
  ): void {
    const ssPtr = this.allocBytes(storeservices);
    const caPtr = this.allocBytes(coreadi);
    const libPtr = this.allocCString(libraryPath);
    const provPtr = this.allocCString(provisioningPath ?? null);
    const idPtr = this.allocCString(identifier ?? null);

    try {
      const result = this.m._anisette_init_from_blobs(
        ssPtr,
        storeservices.length,
        caPtr,
        coreadi.length,
        libPtr,
        provPtr,
        idPtr
      ) as number;
      this.check(result, "anisette_init_from_blobs");
    } finally {
      this.free(ssPtr);
      this.free(caPtr);
      this.free(libPtr);
      this.free(provPtr);
      this.free(idPtr);
    }
  }

  /**
   * Read a file from the WASM virtual filesystem.
   */
  readVirtualFile(filePath: string): Uint8Array {
    const pathPtr = this.allocCString(filePath);
    try {
      const result = this.m._anisette_fs_read_file(pathPtr) as number;
      this.check(result, `anisette_fs_read_file(${filePath})`);
    } finally {
      this.free(pathPtr);
    }
    const ptr = this.m._anisette_fs_read_ptr() as number;
    const len = this.m._anisette_fs_read_len() as number;
    return this.readBytes(ptr, len);
  }

  /**
   * Write a file into the WASM virtual filesystem.
   */
  writeVirtualFile(filePath: string, data: Uint8Array): void {
    const pathPtr = this.allocCString(filePath);
    const dataPtr = this.allocBytes(data);
    try {
      const result = this.m._anisette_fs_write_file(
        pathPtr,
        dataPtr,
        data.length
      ) as number;
      this.check(result, `anisette_fs_write_file(${filePath})`);
    } finally {
      this.free(pathPtr);
      this.free(dataPtr);
    }
  }

  /**
   * Returns 1 if provisioned, 0 if not, throws on error.
   */
  isMachineProvisioned(dsid: bigint): boolean {
    const result = this.m._anisette_is_machine_provisioned(dsid) as number;
    if (result < 0) {
      throw new Error(
        `anisette_is_machine_provisioned: ${this.getLastError()}`
      );
    }
    return result === 1;
  }

  /**
   * Start provisioning — returns CPIM bytes and session handle.
   */
  startProvisioning(
    dsid: bigint,
    spim: Uint8Array
  ): StartProvisioningResult {
    const spimPtr = this.allocBytes(spim);
    try {
      const result = this.m._anisette_start_provisioning(
        dsid,
        spimPtr,
        spim.length
      ) as number;
      this.check(result, "anisette_start_provisioning");
    } finally {
      this.free(spimPtr);
    }

    const cpimPtr = this.m._anisette_get_cpim_ptr() as number;
    const cpimLen = this.m._anisette_get_cpim_len() as number;
    const session = this.m._anisette_get_session() as number;

    return {
      cpim: this.readBytes(cpimPtr, cpimLen),
      session,
    };
  }

  /**
   * Finish provisioning with PTM and TK from Apple servers.
   */
  endProvisioning(session: number, ptm: Uint8Array, tk: Uint8Array): void {
    const ptmPtr = this.allocBytes(ptm);
    const tkPtr = this.allocBytes(tk);
    try {
      const result = this.m._anisette_end_provisioning(
        session,
        ptmPtr,
        ptm.length,
        tkPtr,
        tk.length
      ) as number;
      this.check(result, "anisette_end_provisioning");
    } finally {
      this.free(ptmPtr);
      this.free(tkPtr);
    }
  }

  /**
   * Request OTP — returns OTP bytes and machine ID bytes.
   */
  requestOtp(dsid: bigint): RequestOtpResult {
    const result = this.m._anisette_request_otp(dsid) as number;
    this.check(result, "anisette_request_otp");

    const otpPtr = this.m._anisette_get_otp_ptr() as number;
    const otpLen = this.m._anisette_get_otp_len() as number;
    const midPtr = this.m._anisette_get_mid_ptr() as number;
    const midLen = this.m._anisette_get_mid_len() as number;

    return {
      otp: this.readBytes(otpPtr, otpLen),
      machineId: this.readBytes(midPtr, midLen),
    };
  }

  /**
   * Initialize IDBFS for browser persistence.
   * Only works in browser environments with IDBFS available.
   */
  initIdbfs(path: string): void {
    // Check if FS and IDBFS are available (browser only)
    if (!this.m.FS || !this.m.FS.filesystems?.IDBFS) {
      return; // Node.js or environment without IDBFS
    }

    const normalizedPath = this.normalizeMountPath(path);

    // Create directory structure
    if (normalizedPath !== "/") {
      try {
        this.m.FS.mkdirTree(normalizedPath);
      } catch {
        // Directory already exists, ignore
      }
    }

    // Mount IDBFS
    try {
      this.m.FS.mount(this.m.FS.filesystems.IDBFS, {}, normalizedPath);
    } catch {
      // Already mounted, ignore
    }
  }

  /**
   * Sync IDBFS from IndexedDB to memory (async).
   * Must be called after initIdbfs to load existing data from IndexedDB.
   */
  async syncIdbfsFromStorage(): Promise<void> {
    if (!this.m.FS) {
      return; // FS not available
    }

    return new Promise((resolve, reject) => {
      this.m.FS.syncfs(true, (err: Error | null) => {
        if (err) {
          console.error("[anisette] IDBFS sync from storage failed:", err);
          reject(err);
        } else {
          resolve();
        }
      });
    });
  }

  /**
   * Sync IDBFS from memory to IndexedDB (async).
   * Must be called after modifying files to persist them.
   */
  async syncIdbfsToStorage(): Promise<void> {
    if (!this.m.FS) {
      return; // FS not available
    }

    return new Promise((resolve, reject) => {
      this.m.FS.syncfs(false, (err: Error | null) => {
        if (err) {
          console.error("[anisette] IDBFS sync to storage failed:", err);
          reject(err);
        } else {
          resolve();
        }
      });
    });
  }

  private normalizeMountPath(path: string): string {
    const trimmed = path.trim();
    const noSlash = trimmed.replace(/\/+$/, "");
    const noDot = noSlash.startsWith("./") ? noSlash.slice(2) : noSlash;

    if (!noDot || noDot === ".") {
      return "/";
    } else if (noDot.startsWith("/")) {
      return noDot;
    } else {
      return "/" + noDot;
    }
  }
}
