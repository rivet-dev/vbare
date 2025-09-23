import { defineConfig } from "tsup";
import defaultConfig from "../tsup.base";

export default defineConfig({
  ...defaultConfig,
  entry: ["src/index.ts", "src/cli.ts"],
  outDir: "dist",
});