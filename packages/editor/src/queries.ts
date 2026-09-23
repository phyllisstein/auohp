import { gql } from "@apollo/client";
import type { TypedDocumentNode } from "@apollo/client";
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
} from "./__generated__/queries.gql";


// -----------------------------------------------------------------------------
// Operation documents.
//
// The route builds the editor's executors from these (the persistence
// mutations, the interview-scoped search) and injects them through extension
// config, so the editor imports only the generated result types, never a
// document or a client. Codegen's `documents` glob is `src/**/*.{ts,tsx}`, so
// where a document lives does not change its generated types.
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
        $fragment: String!,
        $interviewUid: String
    ) {
        search {
            statementText(
                fragment: $fragment
                interviewUid: $interviewUid
            ) {
                statement {
                    uid
                    text
                }
            }
        }
    }
`;
