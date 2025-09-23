import { describe, it, expect } from "vitest";
import { VersionedDataHandler, createVersionedDataHandler } from "./index";
import type { VersionedDataConfig } from "./index";

describe("VersionedDataHandler", () => {
	interface TestData {
		name: string;
		value: number;
	}

	const createTestConfig = (
		currentVersion = 1,
	): VersionedDataConfig<TestData> => ({
		currentVersion,
		migrations: new Map(),
		serializeVersion: (data: TestData) => {
			return new TextEncoder().encode(JSON.stringify(data));
		},
		deserializeVersion: (bytes: Uint8Array) => {
			return JSON.parse(new TextDecoder().decode(bytes));
		},
	});

	describe("serializeWithEmbeddedVersion", () => {
		it("should serialize data with embedded version", () => {
			const handler = new VersionedDataHandler(createTestConfig());
			const testData: TestData = { name: "test", value: 42 };

			const serialized = handler.serializeWithEmbeddedVersion(testData);

			expect(serialized).toBeInstanceOf(Uint8Array);
			// First 4 bytes should be version (1 in little endian)
			const versionBytes = Array.from(serialized.slice(0, 4));
			expect(versionBytes).toEqual([1, 0, 0, 0]);
		});
	});

	describe("deserializeWithEmbeddedVersion", () => {
		it("should deserialize data with embedded version", () => {
			const handler = new VersionedDataHandler(createTestConfig());
			const testData: TestData = { name: "test", value: 42 };

			const serialized = handler.serializeWithEmbeddedVersion(testData);
			const deserialized = handler.deserializeWithEmbeddedVersion(serialized);

			expect(deserialized).toEqual(testData);
		});
	});

	describe("migrations", () => {
		it("should apply migrations when deserializing older versions", () => {
			interface V1Data {
				name: string;
			}

			interface V2Data {
				name: string;
				value: number;
			}

			const config: VersionedDataConfig<V2Data> = {
				currentVersion: 2,
				migrations: new Map([
					[1, (data: V1Data): V2Data => ({ ...data, value: 0 })],
				]),
				serializeVersion: (data: V2Data) => {
					return new TextEncoder().encode(JSON.stringify(data));
				},
				deserializeVersion: (bytes: Uint8Array) => {
					return JSON.parse(new TextDecoder().decode(bytes));
				},
			};

			const handler = new VersionedDataHandler(config);

			// Simulate v1 data
			const v1Data: V1Data = { name: "old" };
			const v1Bytes = new TextEncoder().encode(JSON.stringify(v1Data));
			const result = handler.deserialize(v1Bytes, 1);

			expect(result).toEqual({ name: "old", value: 0 });
		});

		it("should throw error for future versions", () => {
			const handler = new VersionedDataHandler(createTestConfig(1));
			const bytes = new Uint8Array();

			expect(() => handler.deserialize(bytes, 2)).toThrow(
				"Cannot decode data from version 2, current version is 1",
			);
		});

		it("should throw error for missing migrations", () => {
			const config = createTestConfig(3);
			const handler = new VersionedDataHandler(config);
			const bytes = new TextEncoder().encode(
				JSON.stringify({ name: "test", value: 1 }),
			);

			expect(() => handler.deserialize(bytes, 1)).toThrow(
				"No migration found from version 1 to 2",
			);
		});
	});

	describe("createVersionedDataHandler", () => {
		it("should create a VersionedDataHandler instance", () => {
			const config = createTestConfig();
			const handler = createVersionedDataHandler(config);

			expect(handler).toBeInstanceOf(VersionedDataHandler);
		});
	});
});

