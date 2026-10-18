/// <reference types="vite/client" />
import { defineConfig } from "vite";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import viteReact from "@vitejs/plugin-react";
import babel from "@rolldown/plugin-babel";
// Imported as a value (not by string name) --- Babel's string resolver would
// rewrite "@preact/signals-react-transform" to the conventional
// "@preact/babel-plugin-signals-react-transform", which does not exist.
import signalsTransform from "@preact/signals-react-transform";
import macros from "unplugin-parcel-macros";
import optimizeLocales from "@react-aria/optimize-locales-plugin";
import svgr from "vite-plugin-svgr";
import { tanstackRouter } from "@tanstack/router-plugin/vite";

// See https://github.com/TanStack/router/discussions/6928#discussioncomment-16147477
function withNormalizedMacroIds (plugin) {
    return {
        ...plugin,
        name: `${ plugin.name }-normalized-ids`,
        transform (code, id) {
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
        // `@vitejs/plugin-react` v6 dropped its `babel` option --- oxc now owns
        // the JSX/TS transform and there is no hook to slot a Babel plugin into.
        // `@preact/signals-react-transform` is Babel-only (no oxc/SWC port), so
        // it runs here as a standalone parse-and-transform pass before oxc: it
        // wraps every component that reads `signal.value` in `useSignals()`
        // bookkeeping, which is what makes bare `.value` reads reactive during
        // render. `mode: "auto"` no-ops on any function that never touches
        // `.value`, so the default include (which, unlike a `$`-anchored regex,
        // also matches TanStack's `route.tsx?tsr-split=...` virtual ids) is
        // safe to leave as-is.
        babel({
            plugins: [[signalsTransform, { mode: "auto" }]],
        }),
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
        // viteReact must come after tanstackStart
        viteReact(),
        {
            ...optimizeLocales.vite({
                locales: ["en-US"],
            }),
            enforce: "pre",
        },
        svgr(),
    ],
    ssr: {
        // styled-components 6 ships no `exports` map --- only legacy `main`
        // (CJS) and `module` (ESM). Left external, SSR loads the CJS build,
        // whose interop puts the callable at `default.default`, so a plain
        // `import styled from "styled-components"` yields the namespace object
        // and `styled.div` is `undefined` ("cannot read properties of undefined
        // (reading 'withConfig')"). Inlining it makes Vite resolve `module`,
        // where the default export is the callable it should be.
        noExternal: [/^@react-spectrum\//, "styled-components"],
    },
    optimizeDeps: {
        exclude: ["@react-spectrum/s2/style"],
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
                manualChunks (id) {
                    if (/macro-(.*)\.css$/.test(id) || /@react-spectrum\/s2\/.*\.css$/.test(id)) {
                        return "s2-styles";
                    }
                },
            },
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
