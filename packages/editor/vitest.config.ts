import { defineConfig } from "vitest/config";
import { playwright } from "@vitest/browser-playwright";

export default defineConfig({
    server: {
        host: "0.0.0.0",
        watch: {
            usePolling: true,
        },
    },
    test: {
        browser: {
            provider: playwright(),
        },
        environment: "jsdom",
        exclude: ["node_modules", "dist", "public"],
        include: ["**/*.test.ts", "**/*.test.tsx"],
        setupFiles: ["vitest-browser-react", "./setup-tests.ts"],
    },
});
