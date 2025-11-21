#!/usr/bin/env node

import { Command } from "commander";
import * as path from "path";
import * as fs from "node:fs/promises";
import { compileSchema } from "./index";

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

async function compileFolder(inputDir: string, outputDir: string, opts: { pedantic?: boolean; generator?: string }) {
  const resolvedInput = path.resolve(inputDir);
  const resolvedOut = path.resolve(outputDir);

  await fs.mkdir(resolvedOut, { recursive: true });

  const entries = await fs.readdir(resolvedInput, { withFileTypes: true });
  const bareFiles = entries
    .filter((e) => e.isFile() && e.name.endsWith(".bare"))
    .map((e) => e.name)
    .sort();

  if (bareFiles.length === 0) {
    console.error(`No .bare files found in ${resolvedInput}`);
    process.exit(1);
  }

  for (const file of bareFiles) {
    const schemaPath = path.join(resolvedInput, file);
    const base = file.replace(/\.bare$/, "");
    const outputPath = path.join(resolvedOut, `${base}.ts`);

    await compileSchema({
      schemaPath,
      outputPath,
      config: {
        pedantic: opts.pedantic ?? false,
        generator: (opts.generator as any) ?? "ts",
        // Support legacy 'string' in fixtures without extra flags
        legacy: true,
      },
    });

    console.log(`Compiled ${schemaPath} -> ${outputPath}`);
  }
}

// Default usage: vbare-compiler <input>
program
  .argument("<input>", "Input .bare file or folder containing .bare files")
  .option("-o, --output <file>", "Output file path (when input is file)")
  .option("-d, --out-dir <dir>", "Output directory (when input is a folder)", "dist")
  .option("--pedantic", "Enable pedantic mode", false)
  .option("--generator <type>", "Generator type (ts, js, dts, bare)", "ts")
  .action(async (input: string, options) => {
    try {
      const inputPath = path.resolve(input);
      if (await isDirectory(inputPath)) {
        await compileFolder(inputPath, options.outDir, {
          pedantic: options.pedantic,
          generator: options.generator,
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
      });

      console.log(`Compiled ${schemaPath} -> ${outputPath}`);
    } catch (error) {
      console.error("Failed to compile:", error);
      process.exit(1);
    }
  });

program.parse();
