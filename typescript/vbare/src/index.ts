// A small wrapper used only for embedded version helpers
export interface VersionedData<T> {
  version: number;
  data: T;
}

// OwnedVersionedData-like config for TypeScript
// Mirrors rust/vbare/src/lib.rs OwnedVersionedData methods.
export interface VersionedDataConfig<S> {
  // Per-version raw (de)serializers
  deserializeVersion: (bytes: Uint8Array, version: number) => any;
  serializeVersion: (data: any, version: number) => Uint8Array;

  // Converter chains.
  // - deserializeConverters[i]: converts from version (i+1) to (i+2)
  // - serializeConverters[i]: converts towards older versions (see notes below)
  deserializeConverters: () => Array<(data: any) => any>;
  serializeConverters: () => Array<(data: any) => any>;
}

// Keep class name for minimal surface change; now generic over S (Self) and L (Latest)
export class VersionedDataHandler<S> {
  constructor(private config: VersionedDataConfig<S>) {}

  // Deserialize bytes of a given version into latest L
  deserialize(bytes: Uint8Array, version: number): S {
    let data: any = this.config.deserializeVersion(bytes, version);
    const converters = this.config.deserializeConverters();
    // Apply converters from `version` onward to reach latest
    for (let i = Math.max(0, version - 1); i < converters.length; i++) {
      data = converters[i](data);
    }
    return data as S;
  }

  // Serialize an S (which may represent any version) to target `version`
  serialize(data: S, version: number): Uint8Array {
    let cur: any = data;
    const converters = this.config.serializeConverters();
    // Apply converters starting from `version - 1` (mirrors Rust skip logic)
    for (let i = Math.max(0, version - 1); i < converters.length; i++) {
      cur = converters[i](cur);
    }
    return this.config.serializeVersion(cur, version);
  }

  // Helpers that embed a u16 (LE) version prefix, like Rust
  serializeWithEmbeddedVersion(data: S, version: number): Uint8Array {
    const payload = this.serialize(data, version);
    const versionBytes = new Uint8Array(2);
    new DataView(versionBytes.buffer).setUint16(0, version, true);
    const out = new Uint8Array(2 + payload.length);
    out.set(versionBytes, 0);
    out.set(payload, 2);
    return out;
  }

  deserializeWithEmbeddedVersion(bytes: Uint8Array): S {
    if (bytes.length < 2) {
      throw new Error("payload too short for embedded version");
    }
    const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const version = dv.getUint16(0, true);
    const payload = bytes.slice(2);
    return this.deserialize(payload, version);
  }

  // Utility kept for completeness in case callers want it.
  private embedVersion(data: VersionedData<Uint8Array>): Uint8Array {
    const versionBytes = new Uint8Array(2);
    new DataView(versionBytes.buffer).setUint16(0, data.version, true);
    const result = new Uint8Array(versionBytes.length + data.data.length);
    result.set(versionBytes);
    result.set(data.data, versionBytes.length);
    return result;
  }

  private extractVersion(bytes: Uint8Array): VersionedData<Uint8Array> {
    if (bytes.length < 2) {
      throw new Error("Invalid versioned data: too short");
    }
    const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const version = dv.getUint16(0, true);
    const data = bytes.slice(2);
    return { version, data };
  }
}

export function createVersionedDataHandler<S>(
  config: VersionedDataConfig<S>,
): VersionedDataHandler<S> {
  return new VersionedDataHandler(config);
}
