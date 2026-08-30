import type { CodegenConfig } from "@graphql-codegen/cli";


const {
    CODEGEN_GRAPHQL_ENDPOINT = "http://api.auohp.localhost/graphql",
} = process.env;


const config: CodegenConfig = {
    schema: CODEGEN_GRAPHQL_ENDPOINT,
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
