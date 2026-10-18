import { defineConfig, mergeConfig } from "vitest/config";
import { playwright } from "@vitest/browser-playwright";
import viteConfig from "./vite.config";

// A standalone `vitest.config.ts` *replaces* `vite.config.ts` rather than
// extending it --- Vitest loads one config file, and this one wins. Merging the
// app config back in is what keeps the React plugin (and so the JSX transform),
// svgr, path resolution and dependency pre-bundling available to tests. Without
// it, `import React from "react"` inside vitest-browser-react resolves to raw
// CJS with no interop shim and fails on the missing default export.
export default mergeConfig(viteConfig, defineConfig({
    server: {
        host: "0.0.0.0",
        watch: {
            usePolling: true,
        },
    },
    test: {
        browser: {
            enabled: true,
            // In Vitest 4 the provider is a factory taking Playwright-level options
            // (launchOptions, contextOptions, ...); the browser to drive is named
            // per-instance below rather than by calling a method on the provider.
            provider: playwright(),
            instances: [
                // `headless` is deliberately unset: it defaults to `process.env.CI`,
                // so runs are headed locally for debugging and headless in CI.
                { browser: "chromium" },
            ],
        },
        // No `environment` here --- browser mode supplies a real DOM. Naming a
        // jsdom/happy-dom environment alongside it is contradictory, and the
        // Node-side environment would be ignored anyway.
        exclude: ["**/node_modules/**", "**/dist/**", "**/public/**"],
        include: ["test/**/*.{test,spec}.{ts,tsx}"],
        setupFiles: ["vitest-browser-react", "./setup-tests.ts"],
    },
}));
