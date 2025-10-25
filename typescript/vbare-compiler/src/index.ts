import { type Config, transform } from "@bare-ts/tools";
import * as fs from "fs/promises";
import * as path from "path";

export interface CompileOptions {
	schemaPath: string;
	outputPath: string;
	config?: Partial<Config>;
}

export async function compileSchema(options: CompileOptions): Promise<void> {
	const { schemaPath, outputPath, config = {} } = options;

	let schema = await fs.readFile(schemaPath, "utf-8");

	// Simple preprocessing: strip lines starting with '//' comments which the parser may not accept
	schema = schema
		.split(/\r?\n/)
		.filter((line) => !line.trimStart().startsWith("//"))
		.join("\n");

	// Normalize map<A, B> -> map<A><B> for parser compatibility
	schema = schema.replace(/map<([^,>]+),\s*([^>]+)>/g, "map<$1><$2>");

	// Convert snake_case field names to camelCase to satisfy semantic checks
	const toCamel = (name: string) =>
		name.replace(/_([a-zA-Z0-9])/g, (_, c: string) => c.toUpperCase());

	// Replace struct field declarations like `  created_at: ...` -> `  createdAt: ...`
	schema = schema.replace(
		/^(\s*)([a-z][a-z0-9_]*)(\s*:\s*)/gim,
		(_m, p1: string, p2: string, p3: string) => {
			return `${p1}${toCamel(p2)}${p3}`;
		},
	);
	const outputDir = path.dirname(outputPath);

	await fs.mkdir(outputDir, { recursive: true });

	const defaultConfig: Partial<Config> = {
		pedantic: true,
		generator: "ts",
		...config,
	};

	let result = transform(schema, defaultConfig);

	result = postProcessAssert(result);

	await fs.writeFile(outputPath, result);
}

const ASSERT_FUNCTION = `
function assert(condition: boolean, message?: string): asserts condition {
    if (!condition) throw new Error(message ?? "Assertion failed")
}

`;

/**
 * Remove Node.js assert import and inject a custom assert function
 */
function postProcessAssert(code: string): string {
	// Remove Node.js assert import
	code = code.replace(/^import assert from "node:assert\/strict"/m, "");

	// INject new assert function
		code += "\n" + ASSERT_FUNCTION;

	return code;
}

export { type Config, transform } from "@bare-ts/tools";
