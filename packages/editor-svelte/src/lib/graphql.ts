// Operation document for the search feature. Types used to be hand-transcribed
// here (spike stub, pre-dating a live API to introspect); now superseded by
// codegen output in __generated__/graphql.gql.ts, generated from this file's
// query string via the near-operation-file preset.
export type {
    SearchAllStatementsQuery,
    SearchAllStatementsQueryVariables,
} from "./__generated__/graphql.gql";

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
