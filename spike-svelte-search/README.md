# spike-svelte-search

SvelteKit port of the editor's **search feature** only. See `../SPIKE-NOTES.md`.

## Run

```
npm install
npm run dev        # → http://localhost:5173/search
```

Open `/search`, type a query, hit Search.

## Pointing at a real GraphQL API

The client (`src/lib/urql.ts`) defaults to `http://api.auohp.localhost/graphql`
(the editor's codegen endpoint). Override:

```
PUBLIC_GRAPHQL_ENDPOINT=http://localhost:XXXX/graphql npm run dev
```

If the API is up but blocks CORS from `:5173`, add a dev proxy in
`vite.config.ts`:

```ts
server: { proxy: { "/graphql": "http://api.auohp.localhost" } }
```

and set the endpoint to `/graphql`.

With no reachable API the page still works — it shows the search UI and surfaces
the network error through the same code path that handles GraphQL errors
(`CombinedError`, the `errorPolicy: "all"` equivalent).

## Not ported

Codegen isn't run — `src/lib/graphql.ts` is hand-transcribed. SSR is off
app-wide (SWC are browser-only). Uses SWC Gen 1 (`@spectrum-web-components/*@1.x`,
`system="spectrum"`) as the deliberate target — see SPIKE-NOTES.
