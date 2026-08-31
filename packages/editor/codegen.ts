import type { CodegenConfig } from "@graphql-codegen/cli";


const {
    CODEGEN_GRAPHQL_ENDPOINT = "http://api.auohp.localhost/graphql",
} = process.env;


const config: CodegenConfig = {
    schema: CODEGEN_GRAPHQL_ENDPOINT,
    documents: ["src/**/*.{ts,tsx}"],
    allowPartialOutputs: true,
    generates: {
        "./src/gql/schema.ts": {
            plugins: ["typescript", "typescript-operations"],
            config: {
                avoidOptionals: true,
                useTypeImports: true,
                maybeValue: "T | null | undefined",
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
