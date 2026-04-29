import { defineConfig } from "vite";
import vinext from "vinext";


export default defineConfig({
    envDir: import.meta.dirname,
    server: {
        allowedHosts: true,
        host: "0.0.0.0",
        port: 3030,
        strictPort: true,
    },
    plugins: [
        vinext(),
    ],
});
