import type { CodegenConfig } from "@graphql-codegen/cli";


const {
    VITE_AUOHP_API_URI: AUOHP_API_URI = "http://localhost:6060",
} = process.env;


const config: CodegenConfig = {
    schema: AUOHP_API_URI + "/graphql",
    documents: ["src/**/*.{ts,tsx}"],
    generates: {
        "./src/gql/": {
            preset: "client",
            config: {
                useTypeImports: true,
            },
        },
        "./schema.graphql": {
            plugins: ["schema-ast"],
            config: {
                includeDirectives: true,
            },
        },
    },
};


export default config;
