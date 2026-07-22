import type { CodegenConfig } from "@graphql-codegen/cli";


const {
    VITE_AUOHP_API_URI: AUOHP_API_URI = "http://localhost:6060",
} = process.env;


const config: CodegenConfig = {
    schema: AUOHP_API_URI + "/graphql",
    documents: ["src/**/*.ts?(x)"],
    generates: {
        "./src/gql/": {
            preset: "client",
            config: {
                useTypeImports: true,
            },
        },
        "./src/gql/schema.ts": {
            plugins: ["typescript", "typescript-operations"],
            config: {
                avoidOptionals: true,
                useTypeImports: true,
                printFieldsOnNewLines: true,
            },
        },
    },
};


export default config;
