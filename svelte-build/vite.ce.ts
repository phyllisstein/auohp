import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { resolve } from "node:path";

export default defineConfig({
    plugins: [svelte({ compilerOptions: { customElement: true } })],
    build: {
        outDir: "dist-ce",
        target: "es2022",
        lib: {
            entry: resolve(import.meta.dirname, "src/entry.ce.js"),
            name: "AuohpSearchCE",
            formats: ["iife"],
            fileName: () => "auohp-search-ce.js",
        },
    },
});
