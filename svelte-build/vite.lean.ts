import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { resolve } from "node:path";

export default defineConfig({
    plugins: [svelte()],
    build: {
        outDir: "dist-lean",
        target: "es2022",
        cssCodeSplit: false,
        lib: {
            entry: resolve(import.meta.dirname, "src/entry.lean.js"),
            name: "AuohpSearchLean",
            formats: ["iife"],
            fileName: () => "auohp-search-lean.js",
        },
    },
});
