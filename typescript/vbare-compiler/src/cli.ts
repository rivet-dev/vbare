#!/usr/bin/env node

import { Command } from "commander";
import * as path from "path";
import * as fs from "node:fs/promises";
import { compileSchema, compileSchemaDirectory } from "./index";

const program = new Command();

program
	.name("vbare-compiler")
	.description("Compile BARE schemas (single file or an entire folder) to TypeScript")
	.version("0.0.4");

async function isDirectory(p: string): Promise<boolean> {
  try {
    const st = await fs.stat(p);
    return st.isDirectory();
  } catch {
    return false;
  }
}

// Default usage: vbare-compiler <input>
program
  .argument("<input>", "Input .bare file or folder containing .bare files")
  .option("-o, --output <file>", "Output file path (when input is file)")
  .option("-d, --out-dir <dir>", "Output directory (when input is a folder)", "dist")
  .option("--pedantic", "Enable pedantic mode", false)
  .option("--generator <type>", "Generator type (ts, js, dts, bare)", "ts")
  .option("--runtime-import <specifier>", "Rewrite @bare-ts/lib imports to this runtime package")
  .action(async (input: string, options) => {
    try {
      const inputPath = path.resolve(input);
      if (await isDirectory(inputPath)) {
        await compileSchemaDirectory({
          inputDir: inputPath,
          outputDir: options.outDir,
          config: {
            pedantic: options.pedantic,
            generator: options.generator,
          },
          runtimeImportPath: options.runtimeImport,
        });
        return;
      }

      // Single file mode
      const schemaPath = inputPath;
      const outputPath = options.output ? path.resolve(options.output) : schemaPath.replace(/\.bare$/, ".ts");

      await compileSchema({
        schemaPath,
        outputPath,
        config: {
          pedantic: options.pedantic ?? false,
          generator: options.generator,
          legacy: true,
        },
        runtimeImportPath: options.runtimeImport,
      });

      console.log(`Compiled ${schemaPath} -> ${outputPath}`);
    } catch (error) {
      console.error("Failed to compile:", error);
      process.exit(1);
    }
  });

program.parse();
