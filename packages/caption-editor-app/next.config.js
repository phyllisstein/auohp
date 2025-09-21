import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

/**
  * @type {import('next').NextConfig}
  */
export default {
    allowedDevOrigins: ["*.auohp.here", "localhost:3000", "127.0.0.1:3000"],
    compiler: {
        styledComponents: {
            displayName: true,
            fileName: true,
            minify: false,
            ssr: true,
        },
    },
    serverRuntimeConfig: {
        host: "0.0.0.0",
    },
    webpack(config, { dev }) {
        config.resolve.enforceExtension = false;
        config.resolve.modules = [
            path.resolve(__dirname, "src"),
            path.resolve(__dirname, "vendor"),
            "node_modules",
            ...config.resolve.modules,
        ];

        config.module.rules.push({
            test: /\.(cql|cypher)$/,
            type: "asset/source",
        });

        return config;
    },
};
