import { Client, cacheExchange, fetchExchange } from "@urql/svelte";

// Endpoint from the editor's codegen.ts default. Override with
// PUBLIC_GRAPHQL_ENDPOINT if the API lives elsewhere.
const url =
    import.meta.env.PUBLIC_GRAPHQL_ENDPOINT ??
    "http://api.auohp.localhost/graphql";

export const client = new Client({
    url,
    exchanges: [cacheExchange, fetchExchange],
    // urql has no per-query errorPolicy knob like Apollo. Its default already
    // does what Apollo's `errorPolicy: "all"` did: a GraphQL error does NOT
    // reject -- `client.query(...).toPromise()` resolves with an
    // OperationResult carrying BOTH `data` (possibly partial) and `error`
    // (a CombinedError). So the load-bearing behaviour is preserved by
    // default; we just read both fields off the result.
    requestPolicy: "network-only",
    // urql defaults queries to GET. This AUOHP API serves the GraphiQL IDE on
    // `GET /graphql` and only answers queries on POST, so force POST.
    preferGetMethod: false,
});
