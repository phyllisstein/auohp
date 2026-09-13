// Operation document for the interview-scoped search query. Feature-scoped
// per PLAN.md sec 1 -- codegen's near-operation-file preset generates
// search-interview/__generated__/queries.gql.ts beside this file.
//
// Ported from packages/editor/src/queries.ts:101-118. Same shape as
// persistence/mutations.ts (deleted, deferred from commit C to here) --
// graphql-tag-pluck finds a `/* GraphQL */`-tagged template literal without
// needing a `gql` import.

export const SEARCH_STATEMENTS_QUERY = /* GraphQL */ `
    query SearchStatements($fragment: String!, $interviewUid: String) {
        search {
            statementText(fragment: $fragment, interviewUid: $interviewUid) {
                statement {
                    uid
                    text
                }
            }
        }
    }
`;
