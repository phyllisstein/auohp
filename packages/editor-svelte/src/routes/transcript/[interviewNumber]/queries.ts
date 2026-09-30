// Operation documents for the transcript route. Ported from
// packages/editor/src/queries.ts -- feature-scoped per PLAN.md sec 1, so
// codegen's near-operation-file preset generates
// transcript/[interviewNumber]/__generated__/queries.gql.ts beside this file.

export const HEADER_QUERY = /* GraphQL */ `
    query Header($interviewNumber: Int!) {
        interview(number: $interviewNumber) {
            interviewee {
                name
            }
        }
    }
`;

export const TRANSCRIPT_QUERY = /* GraphQL */ `
    query Transcript($interviewNumber: Int!) {
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

export const EDIT_STATEMENT_MUTATION = /* GraphQL */ `
    mutation EditStatement($uid: String!, $text: String!, $startTime: Float!, $endTime: Float!) {
        editStatement(input: { uid: $uid, text: $text, startTime: $startTime, endTime: $endTime }) {
            oldHash
            newHash
            statement {
                uid
                text
                startTime
                endTime
            }
        }
    }
`;

export const CREATE_STATEMENT_MUTATION = /* GraphQL */ `
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

export const DESTROY_STATEMENT_MUTATION = /* GraphQL */ `
    mutation DestroyStatement($uid: String!) {
        destroyStatement(uid: $uid) {
            ok
            statement {
                uid
            }
        }
    }
`;
