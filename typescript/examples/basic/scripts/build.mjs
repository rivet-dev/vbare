import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const compilerBin = path.resolve(
  __dirname,
  "../node_modules/.bin/vbare-compiler"
);

// Compile the basic fixtures into ./dist
const inputDir = path.resolve(__dirname, "../../../../fixtures/tests/basic/");

execFileSync(compilerBin, [inputDir, "--out-dir", path.resolve(__dirname, "../dist")], {
  stdio: "inherit",
});
