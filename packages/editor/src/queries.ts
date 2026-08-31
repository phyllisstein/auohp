import { gql } from "@apollo/client";
import type { TypedDocumentNode } from "@graphql-typed-document-node/core";
import type {
    EditStatementMutation,
    EditStatementMutationVariables,
    TranscriptQuery,
    TranscriptQueryVariables,
    SearchStatementsQuery,
    SearchStatementsQueryVariables,
    CreateStatementMutation,
    CreateStatementMutationVariables,
    DestroyStatementMutationVariables,
    DestroyStatementMutation,
} from "./gql/schema";


// -----------------------------------------------------------------------------
// Operation documents.
//
// These lived in the route until the search plumbing moved INSIDE the editor
// extension. `extensions.tsx` now needs SEARCH_STATEMENTS_QUERY, and the route
// already imports `extensions.tsx` --- so leaving the documents in the route
// would close an import cycle. A neutral module breaks it, and co-locating the
// operations is better hygiene anyway: codegen's `documents` glob is
// `src/**/*.{ts,tsx}`, so nothing about the generated types changes.
//
// Worth noting what `graphql()` actually is under the client preset: not a
// runtime parser. It is a lookup into a generated map keyed by the verbatim
// source string, returning a `TypedDocumentNode<Result, Variables>`. That is why
// moving a document between files is free --- the key travels with the text ---
// and why editing one character of the query body requires a codegen run before
// the types resolve again.
// -----------------------------------------------------------------------------

export const EDIT_STATEMENT_MUTATION: TypedDocumentNode<EditStatementMutation, EditStatementMutationVariables> = gql`
    mutation EditStatement($uid: String!, $text: String!, $startTime: Float!, $endTime: Float!) {
        editStatement(input: { uid: $uid, text: $text, startTime: $startTime, endTime: $endTime }) {
            oldHash
            newHash
            wroteEmbedding
            statement {
                uid
                text
                startTime
                endTime
            }
        }
    }
`;

export const CREATE_STATEMENT_MUTATION: TypedDocumentNode<CreateStatementMutation, CreateStatementMutationVariables> = gql`
    mutation CreateStatement($statement: CreateStatementInput!, $interviewUid: String!) {
        createStatement(statement: $statement, interviewUid: $interviewUid) {
            statement {
                uid
                text
                startTime
                endTime
            }
        }
    }
`;

export const DESTROY_STATEMENT_MUTATION: TypedDocumentNode<DestroyStatementMutation, DestroyStatementMutationVariables> = gql`
    mutation DestroyStatement($uid: String!) {
        destroyStatement(uid: $uid) {
            ok
            statement {
                uid
            }
        }
    }
`;

export const TRANSCRIPT_QUERY: TypedDocumentNode<TranscriptQuery, TranscriptQueryVariables> = gql`
    query Transcript($interviewNumber: Int!) {
        health
        interview(number: $interviewNumber) {
            uid
            number
            interviewee {
                uid
                name
            }
            transcript {
                uid
                statements {
                    uid
                    startTime
                    endTime
                    text
                }
            }
            videos {
                uri
            }
        }
    }
`;

export const SEARCH_STATEMENTS_QUERY: TypedDocumentNode<SearchStatementsQuery, SearchStatementsQueryVariables> = gql`
    query SearchStatements(
        $fragment: String!
    ) {
        search {
            statementText(fragment: $fragment) {
                statement {
                    uid
                    text
                }
            }
        }
    }
`;
