import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { compileSchema } from "./index";
import * as fs from "node:fs/promises";
import * as path from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

describe("compileSchema", () => {
	let tempDir: string;
	let outputPath: string;

	// Resolve path to fixtures from this test file location
	const __dirname = path.dirname(fileURLToPath(import.meta.url));
	const fixturesDir = path.resolve(__dirname, "../../../fixtures/tests/basic");
	const v1Schema = path.join(fixturesDir, "v1.bare");
	const v2Schema = path.join(fixturesDir, "v1.bare");

	beforeAll(async () => {
		// Create temporary directory for test outputs
		tempDir = await fs.mkdtemp(path.join(tmpdir(), "vbare-compiler-test-"));
		outputPath = path.join(tempDir, "output.ts");
	});

	afterAll(async () => {
		// Clean up temporary directory
		await fs.rm(tempDir, { recursive: true, force: true });
	});

	it("should compile a simple BARE schema from fixtures", async () => {
		// Compile the v1 fixture schema
		await compileSchema({
			schemaPath: v1Schema,
			outputPath,
			config: {
				legacy: true,
			},
		});

		// Check that output file was created
		const outputExists = await fs
			.access(outputPath)
			.then(() => true)
			.catch(() => false);
		expect(outputExists).toBe(true);

		// Check that output contains TypeScript code and known types
		const output = await fs.readFile(outputPath, "utf-8");
		expect(output).toContain("export");
		expect(output).toContain("Todo");
		expect(output).toContain("App");
	});

	it("should handle custom config options using fixtures", async () => {
		await compileSchema({
			schemaPath: v2Schema,
			outputPath,
			config: {
				generator: "ts",
				pedantic: false,
				legacy: true,
			},
		});

		const output = await fs.readFile(outputPath, "utf-8");
		// Ensure generator + config produce valid TS helpers
		expect(output).toContain("export");
		expect(output).toContain("readTodo");
		expect(output).toContain("writeTodo");
	});
});
