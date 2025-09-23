#!/usr/bin/env node

import { Command } from "commander";
import * as path from "path";
import { compileSchema } from "./index";

const program = new Command();

program
	.name("bare-compiler")
	.description("Compile BARE schemas to TypeScript")
	.version("0.0.1");

program
	.command("compile")
	.description("Compile a BARE schema file")
	.argument("<input>", "Input BARE schema file")
	.option("-o, --output <file>", "Output file path")
	.option("--pedantic", "Enable pedantic mode", false)
	.option("--generator <type>", "Generator type (ts, js, dts, bare)", "ts")
	.action(async (input: string, options) => {
		try {
			const schemaPath = path.resolve(input);
			const outputPath = options.output
				? path.resolve(options.output)
				: schemaPath.replace(/\.bare$/, ".ts");

			await compileSchema({
				schemaPath,
				outputPath,
				config: {
					pedantic: options.pedantic,
					generator: options.generator,
				},
			});

			console.log(`Successfully compiled ${input} to ${outputPath}`);
		} catch (error) {
			console.error("Failed to compile schema:", error);
			process.exit(1);
		}
	});

program.parse();

