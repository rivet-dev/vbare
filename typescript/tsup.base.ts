import type { Options } from "tsup";

export default {
  target: "node16",
  platform: "node",
  format: ["cjs", "esm"],
  sourcemap: true,
  clean: true,
  dts: {
    compilerOptions: {
      skipLibCheck: true,
      resolveJsonModule: true,
    },
  },
  minify: false,
  splitting: true,
  skipNodeModulesBundle: true,
  publicDir: true,
} satisfies Options;