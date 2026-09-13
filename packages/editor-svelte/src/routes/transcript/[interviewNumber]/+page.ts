import { error } from "@sveltejs/kit";
import { client } from "$lib/urql";
import { HEADER_QUERY, TRANSCRIPT_QUERY } from "./queries";
import type {
    HeaderQuery,
    HeaderQueryVariables,
    TranscriptQuery,
    TranscriptQueryVariables,
} from "./__generated__/queries.gql";
import type { PageLoad } from "./$types";

// Port of $interviewNumber.tsx's loader. Apollo's preloadQuery (a Suspense
// resource read later via useReadQuery) has no urql equivalent worth
// reaching for here -- SvelteKit's load function already IS the async
// boundary, so both queries are simply awaited before the page renders.
export const load: PageLoad = async ({ params, fetch }) => {
    const interviewNumber = Number.parseInt(params.interviewNumber);
    if (Number.isNaN(interviewNumber)) {
        error(400, "Invalid interview number");
    }

    const [headerResult, transcriptResult] = await Promise.all([
        client
            .query<HeaderQuery, HeaderQueryVariables>(HEADER_QUERY, { interviewNumber }, { fetch })
            .toPromise(),
        client
            .query<TranscriptQuery, TranscriptQueryVariables>(TRANSCRIPT_QUERY, { interviewNumber }, { fetch })
            .toPromise(),
    ]);

    return {
        interviewNumber,
        header: headerResult.data ?? null,
        transcript: transcriptResult.data ?? null,
    };
};
