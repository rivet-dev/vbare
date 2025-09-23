import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { compileSchema } from "./index";
import * as fs from "node:fs/promises";
import * as path from "node:path";
import { tmpdir } from "node:os";

describe("compileSchema", () => {
	let tempDir: string;
	let schemaPath: string;
	let outputPath: string;

	beforeAll(async () => {
		// Create temporary directory for test files
		tempDir = await fs.mkdtemp(path.join(tmpdir(), "vbare-compiler-test-"));
		schemaPath = path.join(tempDir, "test.bare");
		outputPath = path.join(tempDir, "output.ts");
	});

	afterAll(async () => {
		// Clean up temporary directory
		await fs.rm(tempDir, { recursive: true, force: true });
	});

	it("should compile a simple BARE schema", async () => {
		// Create a simple BARE schema
		const schema = `type Person struct {
  name: str
  age: u8
}`;
		await fs.writeFile(schemaPath, schema);

		// Compile the schema
		await compileSchema({
			schemaPath,
			outputPath,
		});

		// Check that output file was created
		const outputExists = await fs
			.access(outputPath)
			.then(() => true)
			.catch(() => false);
		expect(outputExists).toBe(true);

		// Check that output contains TypeScript code
		const output = await fs.readFile(outputPath, "utf-8");
		expect(output).toContain("export");
		expect(output).toContain("Person");
	});

	it("should handle custom config options", async () => {
		const schema = `type Status enum {
  ACTIVE
  INACTIVE
}`;
		await fs.writeFile(schemaPath, schema);

		await compileSchema({
			schemaPath,
			outputPath,
			config: {
				generator: "ts",
				pedantic: false,
			},
		});

		const output = await fs.readFile(outputPath, "utf-8");
		expect(output).toContain("Status");
		// The output uses PascalCase for enum values
		expect(output).toContain("Active");
		expect(output).toContain("Inactive");
	});
});

