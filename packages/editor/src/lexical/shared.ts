import { useLazyQuery, useMutation } from "@apollo/client/react";
import type { EditStatementMutation, EditStatementMutationVariables, TranscriptQuery, SearchStatementsQuery, SearchStatementsQueryVariables, CreateStatementMutation, CreateStatementMutationVariables, DestroyStatementMutationVariables, DestroyStatementMutation } from "~/__generated__/queries.gql";


// A split-on-Enter produces a second statement the backend knows nothing about:
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
export type TranscriptStatements = TranscriptQuery["interview"]["transcript"]["statements"];
export type EditStatementFn = ReturnType<typeof useMutation<EditStatementMutation, EditStatementMutationVariables>>[0];

// Only the data shape survives. The executor and state-tuple aliases existed
// solely to type the search plumbing when the route owned `useLazyQuery` and
// pushed its result through extension config --- a channel that cannot deliver
// updates, because `build` runs once. The query now lives inside the extension,
// so the only thing crossing a boundary is the result payload itself.
//
// Note the indexed access doing the work: `[1]["data"]` walks the tuple Apollo
// returns and pulls the field off it, so this alias tracks any future change to
// `useLazyQuery`'s result type without us restating it.
export type SearchStatementsData = ReturnType<typeof useLazyQuery<SearchStatementsQuery, SearchStatementsQueryVariables>>[1]["data"];
export type DestroyStatementFn = ReturnType<typeof useMutation<DestroyStatementMutation, DestroyStatementMutationVariables>>[0];
export type CreateStatementFn = ReturnType<typeof useMutation<CreateStatementMutation, CreateStatementMutationVariables>>[0];

// Derived from the operation's variables rather than imported as the standalone
// `CreateStatementInput`, for the same reason as the aliases above: the indexed
// access tracks the mutation's actual signature, so renaming the argument or
// tightening the input shape surfaces here instead of drifting silently.
export type CreateStatementInput = CreateStatementMutationVariables["statement"];
