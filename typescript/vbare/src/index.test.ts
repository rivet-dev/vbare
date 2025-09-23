import { describe, it, expect } from "vitest";
import {
  VersionedDataHandler,
  createVersionedDataHandler,
  type VersionedDataConfig,
} from "./index";

describe("VersionedDataHandler (OwnedVersionedData API)", () => {
  type V1 = { id: number; name: string };
  type V2 = { id: number; name: string; description: string };

  type S = V2;

  const enc = (o: any) => new TextEncoder().encode(JSON.stringify(o));
  const dec = (b: Uint8Array) => JSON.parse(new TextDecoder().decode(b));

  const v1_to_v2 = (x: any): V2 => {
    const v1 = x as V1;
    return { id: v1.id, name: v1.name, description: "default" };
  };
  const v2_to_v1 = (x: any): V1 => {
    const v2 = x as V2;
    return { id: v2.id, name: v2.name };
  };

  const config: VersionedDataConfig<S> = {
    deserializeVersion: (bytes: Uint8Array, version: number): any => {
      switch (version) {
        case 1:
          return dec(bytes) as V1;
        case 2:
          return dec(bytes) as V2;
        default:
          throw new Error(`invalid version: ${version}`);
      }
    },
    serializeVersion: (s: any, _version: number): Uint8Array => {
      return enc(s);
    },
    deserializeConverters: () => [v1_to_v2],
    serializeConverters: () => [v2_to_v1],
  };

  it("serialize v2 -> v1 -> deserialize -> v2 (converts)", () => {
    const handler = new VersionedDataHandler(config);
    const latest: V2 = { id: 456, name: "test", description: "will be stripped" };
    const payloadV1 = handler.serialize(latest, 1);
    const decoded = handler.deserialize(payloadV1, 1);
    expect(decoded).toEqual({ id: 456, name: "test", description: "default" });
  });

  it("serialize v2 -> v2 -> deserialize -> v2 (no change)", () => {
    const handler = new VersionedDataHandler(config);
    const latest: V2 = { id: 456, name: "test", description: "data" };
    const payloadV2 = handler.serialize(latest, 2);
    const decoded = handler.deserialize(payloadV2, 2);
    expect(decoded).toEqual(latest);
  });

  it("embedded v2->v1->v2", () => {
    const handler = new VersionedDataHandler(config);
    const latest: V2 = { id: 456, name: "test", description: "will be stripped" };
    const payload = handler.serializeWithEmbeddedVersion(latest, 1);
    expect(payload[0]).toBe(1);
    expect(payload[1]).toBe(0);
    const decoded = handler.deserializeWithEmbeddedVersion(payload);
    expect(decoded).toEqual({ id: 456, name: "test", description: "default" });
  });

  it("embedded v2->v2", () => {
    const handler = new VersionedDataHandler(config);
    const latest: V2 = { id: 456, name: "test", description: "data" };
    const payload = handler.serializeWithEmbeddedVersion(latest, 2);
    expect(payload[0]).toBe(2);
    expect(payload[1]).toBe(0);
    const decoded = handler.deserializeWithEmbeddedVersion(payload);
    expect(decoded).toEqual(latest);
  });

  it("unsupported version", () => {
    const handler = new VersionedDataHandler(config);
    expect(() => handler.deserialize(new Uint8Array(), 99)).toThrow(
      "invalid version: 99",
    );
  });

  it("no converters still works for single version", () => {
    type S1 = V1;
    const cfg1: VersionedDataConfig<S1> = {
      deserializeVersion: (bytes: Uint8Array, version: number): any => {
        if (version !== 1) throw new Error(`invalid version: ${version}`);
        return dec(bytes) as V1;
      },
      serializeVersion: (s: any) => enc(s),
      deserializeConverters: () => [],
      serializeConverters: () => [],
    };
    const handler = new VersionedDataHandler(cfg1);
    const data: V1 = { id: 1, name: "x" };
    const payload = handler.serialize(data, 1);
    const decoded = handler.deserialize(payload, 1);
    expect(decoded).toEqual(data);
  });

  it("factory returns handler", () => {
    const handler = createVersionedDataHandler(config);
    expect(handler).toBeInstanceOf(VersionedDataHandler);
  });
});
