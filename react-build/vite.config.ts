import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { resolve } from "node:path";

export default defineConfig({
    envDir: import.meta.dirname,
    resolve: {
        alias: {
            "~": resolve(import.meta.dirname, "src"),
        },
    },
    plugins: [react()],
    define: {
        "import.meta.env.VITE_GRAPHQL_URI": JSON.stringify("https://api.auohp.example/graphql"),
    },
    build: {
        outDir: "dist",
        target: "es2022",
        lib: {
            entry: resolve(import.meta.dirname, "src/widget-entry.tsx"),
            name: "AuohpSearch",
            formats: ["iife"],
            fileName: () => "auohp-search.js",
        },
        rollupOptions: {
            output: { inlineDynamicImports: true },
        },
    },
    oxc: {
        plugins: {
            styledComponents: {
                transpileTemplateLiterals: false,
                minify: false,
            },
        },
    },
});
