import { resolve } from "path";
import { defineConfig } from "vitest/config";
import defaultConfig from "../vitest.base";

export default defineConfig({
	...defaultConfig,
	resolve: {
		alias: {
			"@": resolve(__dirname, "./src"),
		},
	},
});