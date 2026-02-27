// LibraryStore — holds the two required Android .so blobs

const REQUIRED_LIBS = [
  "libstoreservicescore.so",
  "libCoreADI.so",
] as const;

export type LibraryName = (typeof REQUIRED_LIBS)[number];

export class LibraryStore {
  private libs: Map<LibraryName, Uint8Array>;

  private constructor(libs: Map<LibraryName, Uint8Array>) {
    this.libs = libs;
  }

  static fromBlobs(
    storeservicescore: Uint8Array,
    coreadi: Uint8Array
  ): LibraryStore {
    const map = new Map<LibraryName, Uint8Array>();
    map.set("libstoreservicescore.so", storeservicescore);
    map.set("libCoreADI.so", coreadi);
    return new LibraryStore(map);
  }

  get(name: LibraryName): Uint8Array {
    const data = this.libs.get(name);
    if (!data) throw new Error(`Library not loaded: ${name}`);
    return data;
  }

  get storeservicescore(): Uint8Array {
    return this.get("libstoreservicescore.so");
  }

  get coreadi(): Uint8Array {
    return this.get("libCoreADI.so");
  }
}
