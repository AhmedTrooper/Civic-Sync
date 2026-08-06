import tailwindcss from "@tailwindcss/vite";
import { devtools } from "@tanstack/devtools-vite";

import { tanstackStart } from "@tanstack/react-start/plugin/vite";

import viteReact from "@vitejs/plugin-react";
import { nitro } from "nitro/vite";
import { defineConfig } from "vite";

const config = defineConfig({
	resolve: { tsconfigPaths: true },
	plugins: [
		devtools(),
		nitro({ 
			rollupConfig: { external: [/^@sentry\//] },
			routeRules: {
				'/api/**': { proxy: 'http://127.0.0.1:8080/api/**' }
			}
		}),
		tailwindcss(),
		tanstackStart(),
		viteReact(),
	],
	server: {
		proxy: {
			'/api': {
				target: 'http://127.0.0.1:8080',
				changeOrigin: true,
			}
		}
	}
});

export default config;
