import { defineConfig } from "tsup";
import defaultConfig from "../tsup.base";

export default defineConfig({
  ...defaultConfig,
  entry: ["src/index.ts"],
  outDir: "dist",
});