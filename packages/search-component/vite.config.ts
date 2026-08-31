import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";


export default defineConfig({
    appType: "spa",
    envDir: import.meta.dirname,
    resolve: {
        tsconfigPaths: true,
    },
    plugins: [
        react(),
    ],
    publicDir: "../public",
    root: "./src",
    server: {
        allowedHosts: true,
        host: "0.0.0.0",
        port: 4040,
        strictPort: true,
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
