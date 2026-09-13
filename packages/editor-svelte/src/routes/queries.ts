// Operation document for the interview list. Ported from the near-duplicate
// LIST_INTERVIEWS_QUERY in packages/editor/src/routes/index.tsx and
// routes/transcript/index.tsx -- collapsed to one route, one query.

export const LIST_INTERVIEWS_QUERY = /* GraphQL */ `
    query ListInterviews {
        interviews {
            number
            interviewee {
                name
            }
        }
    }
`;
