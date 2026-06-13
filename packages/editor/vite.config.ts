/// <reference types="vite/client" />
import { defineConfig } from "vite";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import viteReact from "@vitejs/plugin-react";
import macros from "unplugin-parcel-macros";

export default defineConfig({
    envDir: import.meta.dirname,
    resolve: {
        tsconfigPaths: true,
    },
    server: {
        allowedHosts: true,
        host: "0.0.0.0",
        port: 3030,
        strictPort: true,
    },
    plugins: [
        macros.vite(),
        tanstackStart(),
        // viteReact must come after tanstackStart
        viteReact(),
    ],
    ssr: {
        noExternal: [/^@react-spectrum\//],
    },
    build: {
        target: ["es2022"],
        // Lightning CSS produces a much smaller CSS bundle than the default minifier.
        cssMinify: "lightningcss",
        rollupOptions: {
            output: {
                // Bundle all S2 and style-macro generated CSS into a single bundle instead of code splitting.
                // Because atomic CSS has so much overlap between components, loading all CSS up front results in
                // smaller bundles instead of producing duplication between pages.
                manualChunks(id) {
                    if (/macro-(.*)\.css$/.test(id) || /@react-spectrum\/s2\/.*\.css$/.test(id)) {
                        return "s2-styles";
                    }
                },
            },
        },
    },
});
