import { defineConfig } from "vite"
import { devtools } from "@tanstack/devtools-vite"

import { tanstackStart } from "@tanstack/react-start/plugin/vite"

import viteReact from "@vitejs/plugin-react"

const config = defineConfig({
    resolve: { tsconfigPaths: true },
    plugins: [
        devtools(),
        tanstackStart({
            router: {
                routeFileIgnorePattern: `(\\.styles\\.(ts|tsx)$)|(__generated__)`,
                quoteStyle: "double",
                semicolons: true,
            },
            vite: {
                installDevServerMiddleware: true,
            },
        }),
        viteReact(),
  ],
});

export default config;
