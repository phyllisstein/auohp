import type { CodegenConfig } from "@graphql-codegen/cli";


const {
    CODEGEN_GRAPHQL_ENDPOINT = "http://api.auohp.localhost/graphql",
} = import.meta.env;


const config: CodegenConfig = {
    schema: CODEGEN_GRAPHQL_ENDPOINT,
    documents: ["src/**/!(*.gql).{ts,tsx}"],
    allowPartialOutputs: true,
    generates: {
        "./src/__generated__/schema.gql.ts": {
            plugins: ["typescript"],
            config: {
                avoidOptionals: true,
                useTypeImports: true,
                extractAllFieldsToTypesCompact: true,
            },
            hooks: {
                afterOneFileWrite: ["oxlint --fix"],
            },
        },
        "./src/": {
            preset: "near-operation-file",
            presetConfig: {
                extension: ".gql.ts",
                baseTypesPath: "__generated__/schema.gql.ts",
                folder: "__generated__",
            },
            plugins: ["typescript-operations"],
            config: {
                avoidOptionals: true,
                useTypeImports: true,
                extractAllFieldsToTypesCompact: true,
            },
            hooks: {
                afterOneFileWrite: ["oxlint --fix"],
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
