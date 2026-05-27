import { nodeResolve } from "@rollup/plugin-node-resolve";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";


export default defineConfig({
    appType: "spa",
    envDir: import.meta.dirname,
    plugins: [
        react(),
        nodeResolve({
            extensions: [".tsx", ".ts", ".js"],
            moduleDirectories: ["src", "node_modules"],
        }),
    ],
    publicDir: "../public",
    root: "./src",
    server: {
        allowedHosts: true,
        host: "0.0.0.0",
        port: 4040,
        strictPort: true,
    },
});
