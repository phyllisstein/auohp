import { useMutation } from "@apollo/client/react";
import type { EditStatementMutation, EditStatementMutationVariables, TranscriptQuery } from "@/gql/graphql";


// A split-on-Enter produces a SECOND statement the backend knows nothing about:
// there is no `splitStatement`/`createStatement` mutation, only `editStatement`
// keyed by an existing uid. We tag synthetic uids with this marker so the
// persistence seam can recognise --- and skip --- statements that would 404.
// BACKEND GAP: a `splitStatement(uid, atOffset)` mutation would let these persist.
export const SYNTHETIC_UID_MARKER = "::split-";


export const formatTimestamp = (timestamp: number) =>
    Temporal.Duration.from({ seconds: Math.round(timestamp) })
        .round({
            largestUnit: "hours",
            smallestUnit: "seconds",
        })
        .toLocaleString("en-US", {
            style: "digital",
            hoursDisplay: "auto",
            hours: "numeric",
        });


// ---- GraphQL result-shape aliases (derived from the reused operations) --------
export type TranscriptStatements = TranscriptQuery["interviewTranscript"]["statements"];
export type EditStatementFn = ReturnType<typeof useMutation<EditStatementMutation, EditStatementMutationVariables>>[0];
