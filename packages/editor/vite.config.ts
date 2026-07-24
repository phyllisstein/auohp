/// <reference types="vite/client" />
import { defineConfig } from "vite";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import viteReact from "@vitejs/plugin-react-swc";
import macros from "unplugin-parcel-macros";
import optimizeLocales from "@react-aria/optimize-locales-plugin";

// See https://github.com/TanStack/router/discussions/6928#discussioncomment-16147477
function withNormalizedMacroIds(plugin) {
    return {
        ...plugin,
        name: `${ plugin.name }-normalized-ids`,
        transform(code, id) {
            return plugin.transform?.call(this, code, id.replace(/\?.*$/, ""));
        },
    };
}


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
        withNormalizedMacroIds(macros.vite()), // Must come first!
        tanstackStart(),
        // viteReact must come after tanstackStart
        viteReact({
            plugins: [
                [
                    "@swc-contrib/plugin-graphql-codegen-client-preset",
                    { artifactDirectory: "./src/gql", gqlTagName: "graphql" },
                ],
            ],
        }),
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
