import { client } from "$lib/urql";
import { LIST_INTERVIEWS_QUERY } from "./queries";
import type { ListInterviewsQuery, ListInterviewsQueryVariables } from "./__generated__/queries.gql";
import type { PageLoad } from "./$types";

// Port of the loader shared by routes/index.tsx and routes/transcript/index.tsx
// (collapsed to one route -- see MIGRATION.md's known-defects list). Apollo's
// preloadQuery/useReadQuery pair has no urql equivalent worth reaching for;
// SvelteKit's load function is already the async boundary.
export const load: PageLoad = async ({ fetch }) => {
    const result = await client
        .query<ListInterviewsQuery, ListInterviewsQueryVariables>(LIST_INTERVIEWS_QUERY, {}, { fetch })
        .toPromise();

    return {
        interviews: result.data?.interviews ?? [],
    };
};
