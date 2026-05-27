import type { NextConfig } from "next";

const config: NextConfig = {
    compiler: {
        styledComponents: {
            displayName: true,
            fileName: true,
            minify: false,
            ssr: true,
        },
    },
};

export default config;
