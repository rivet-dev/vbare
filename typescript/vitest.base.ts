import type { ViteUserConfig } from "vitest/config";

export default {
	test: {
		testTimeout: 5_000,
		hookTimeout: 5_000,
		globals: true,
		environment: "node",
		coverage: {
			provider: "v8",
			reporter: ["text", "json", "html"],
		},
	},
} satisfies ViteUserConfig;