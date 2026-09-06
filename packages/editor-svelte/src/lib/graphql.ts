// Hand-transcribed from packages/editor/src/routes/search/__generated__/index.gql.ts
// plus the operation in route.tsx. In the real port this is graphql-codegen
// output: the schema + near-operation-file-preset are framework-neutral, so
// `npm run codegen` against schema.graphql regenerates these verbatim. Stubbed
// here to avoid running codegen in a spike (needs the API up for schema
// introspection, or a local schema.graphql copy).

export type SearchAllStatementsQuery = {
    search: {
        statementText: Array<{
            statement: {
                uid: string;
                text: string;
                startTime: number | null;
                endTime: number | null;
            };
            interview: {
                uid: string;
                number: number;
                interviewee: { uid: string; name: string };
            };
        }>;
    };
};

export type SearchAllStatementsQueryVariables = {
    fragment: string;
};

export const SEARCH_ALL_STATEMENTS_QUERY = /* GraphQL */ `
    query SearchAllStatements($fragment: String!) {
        search {
            statementText(fragment: $fragment) {
                statement {
                    uid
                    text
                    startTime
                    endTime
                }
                interview {
                    uid
                    number
                    interviewee {
                        uid
                        name
                    }
                }
            }
        }
    }
`;
