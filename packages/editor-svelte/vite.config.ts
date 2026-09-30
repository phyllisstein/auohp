/// <reference types="vite/client" />
import { defineConfig } from "vite";
import macros from "unplugin-parcel-macros";
import { sveltekit } from '@sveltejs/kit/vite';
import adapter from '@sveltejs/adapter-auto';

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
    resolve: {
        tsconfigPaths: true,
    },
    server: {
        allowedHosts: true,
        host: "0.0.0.0",
        port: 2020,
        strictPort: true,
    },
    plugins: [
        withNormalizedMacroIds(macros.vite()), // Must come first!
        sveltekit({
            compilerOptions: {
                // Force runes mode for the project, except for libraries. Can be removed in svelte 6.
                runes: ({ filename }) =>
                    filename.split(/[/\\]/).includes('node_modules') ? undefined : true,
                customElement: true,
                experimental: {
                    async: true,
                }
            },
            experimental: {
                remoteFunctions: true,
            },
            alias: {
                "$styles": "./src/styles",
            },

            // adapter-auto only supports some environments, see https://svelte.dev/docs/kit/adapter-auto for a list.
            // If your environment is not supported, or you settled on a specific environment, switch out the adapter.
            // See https://svelte.dev/docs/kit/adapters for more information about adapters.
            adapter: adapter()
        }),
    ],
});
