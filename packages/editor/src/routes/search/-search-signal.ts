import { createModel, signal } from "@preact/signals-react";
import type { SearchAllStatementsQuery } from "./__generated__/index.gql";

// `createModel` mints a constructor whose instances own the signals returned by
// the factory. It is *not* itself a signal --- `searchQuery.query` is the
// `Signal<string>`, `searchQuery.query.value` is the string. Reading those
// `.value`s inside a component is what subscribes it, courtesy of the
// signals-react-transform Babel pass wired into vite.config.ts.
export const SearchQuery = createModel(() => {
    const query = signal<string>("");
    const results = signal<SearchAllStatementsQuery | null>(null);
    const loading = signal<boolean>(false);
    const error = signal<Error | null>(null);

    return { query, results, loading, error };
});

// One shared instance is the single source of truth for the whole search
// feature. Both the parent `/search` route and the `/search/results` child
// import this and read/write it directly --- no context, no router state, no
// second standalone signal.
export const searchQuery = new SearchQuery();
