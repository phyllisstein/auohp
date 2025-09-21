import { defineConfig } from "vitest/config";

export default defineConfig({
    server: {
        host: "0.0.0.0",
        watch: {
            usePolling: true,
        },
    },
    test: {
        browser: {
            enabled: false,
            headless: true,
            name: "chromium",
            provider: "playwright",
        },
        environment: "jsdom",
        exclude: ["node_modules", "dist", "public"],
        include: ["**/*.test.ts", "**/*.test.tsx"],
        setupFiles: ["vitest-browser-react", "./setup-tests.ts"],
    },
});
