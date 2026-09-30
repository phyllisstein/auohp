// Port of packages/editor/src/routes/search/-search-signal.ts
//
// The original: a `@preact/signals-react` `createModel` singleton whose fields
// are `Signal<T>`, read via `.value`, made reactive by a Babel transform.
//
// Svelte 5: a plain module holding `$state` runes. `.svelte.ts` extension is
// required for runes outside a component. No transform, no `.value` -- reads and
// writes are just property access, and the compiler tracks them. The whole
// `@rolldown/plugin-babel` + `@preact/signals-react-transform` apparatus in the
// original vite.config.ts has no equivalent here: it evaporates.
//
// One shared instance = single source of truth. Both the search page and the
// results view import this object and read/write it directly. No context, no
// router state.

import type { CombinedError } from "@urql/svelte";
import type { SearchAllStatementsQuery } from "./graphql";

export const searchQuery = $state({
    query: "",
    results: null as SearchAllStatementsQuery | null,
    loading: false,
    // urql surfaces partial-data-plus-errors as CombinedError, which is what
    // replaces Apollo's `errorPolicy: "all"` behaviour (see runSearch below).
    error: null as CombinedError | null,
});
