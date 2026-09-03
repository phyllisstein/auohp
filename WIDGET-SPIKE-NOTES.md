# Widget injection spike — React vs Svelte

Quick-and-dirty. All builds are IIFE lib-mode, esbuild-minified, `target: es2022`.
Widget = the search box + results dropdown only (player excluded from the baseline).

## What the widget actually is (React, today)

- Entry `src/index.tsx` mounts via `createRoot` into `#search-component`; wrapped in
  `<StrictMode><ApolloProvider>`. There is **no build script** in the package —
  only `dev` + `codegen`. `dist/` held only a stale `tsconfig.tsbuildinfo`. So
  "the React build" did not exist; I wrote a lib-mode `vite build` for it.
- Source files ported: `search.tsx`, `results.tsx`, `use-portal.tsx`,
  `search-styles.ts`, `results-styles.ts`, `__generated__/search.gql.ts`.
- State: `useState`/`useRef`/`useEffect` for the input-rect measurement,
  `useLazyQuery` (Apollo) fired on every `onChange`, `window.location.href` for
  navigation. **No rxjs, no react-router, no query-string in the search widget** —
  those live in `index.tsx`/player only and tree-shook out. `query-string` is
  imported by `search.tsx` but only for `stringifyUrl` on result click.
- styled-components: used **only internally** (`-styles.ts` files). The external
  surface exposes exactly **one** semantic className hook: `search-container`.
  Everything else a downstream designer can target is a styled-components hash
  class (`.sc-xxxxx`) — stable per build, not a documented API.
- `use-portal.tsx` + `createPortal` push the results list to `document.body` as
  `position: fixed`, using `useId()` for the portal root id.

## The three (four) artifacts

| Artifact | Raw JS | Gzip JS | +CSS (gz) | Files | Modules | GraphQL layer |
|---|---|---|---|---|---|---|
| **React baseline** | 814 KB | **248 KB** | 0 (CSS-in-JS) | 1 | 618 | `@apollo/client` 4 + `graphql` 17 |
| **Svelte custom-element** | 91 KB | **28 KB** | 0 (inlined) | 1 | 246 | `graphql-request` 7 (pulls `graphql` 16 parser) |
| **Svelte mount-to-div** | 84 KB | **25 KB** | +0.4 KB | 2 | 247 | `graphql-request` 7 |
| **Svelte lean (fetch)** | 33 KB | **13 KB** | +0.4 KB | 2 | 108 | none — hand-rolled `fetch` POST |

Gzip numbers: React is Vite's reported 247.9 KB (my `gzip -9` gave 244.6 KB).
Svelte numbers are `gzip -9` of the emitted files.

### Where the bytes are

- **React**: react-dom (~130 KB raw min) + Apollo Client 4 `InMemoryCache` +
  `graphql` 17 (full document parser + a chunk of the executor via `gql`). None
  of that is optional with the current architecture.
- **Svelte CE/div**: `graphql` 16's `parse()` (~45–50 KB raw) dragged in by
  `graphql-request`. `graphql-request` itself is tiny; the cost is that it
  `parse()`s the query string. Svelte 5 runtime for this component is ~10–12 KB gz.
- **Svelte lean**: drop `graphql-request`, POST `{query, variables}` with `fetch`,
  read `json.data`. Bundle collapses to the Svelte runtime + ~1 KB of component.
  This is the honest Svelte floor and it's **~18x smaller than React, gzipped.**

## Injection snippet a CMS editor pastes

**React baseline**
```html
<div id="auohp-search"></div>
<script src="https://cdn.example/auohp-search.js" defer></script>
```
One script, one div. But the script is 814 KB / 239 KB gz. Coexistence risk: if
the host page (WordPress) already loads React (many block themes / plugins do),
you now ship a second React 19 + a second ReactDOM into the page. They don't
conflict (separate closures) but it's dead weight and two reconcilers.

**Svelte custom-element**
```html
<script src="https://cdn.example/auohp-search-ce.js" defer></script>
<auohp-search></auohp-search>
```
One script, one tag, CSS auto-injected. Cleanest paste. Element upgrades whenever
it appears in the DOM (even if injected later by the CMS).

**Svelte mount-to-div** (and lean)
```html
<div id="auohp-search"></div>
<script src="https://cdn.example/auohp-search-div.js" defer></script>
<link rel="stylesheet" href="https://cdn.example/auohp-search-svelte.css">
```
Two or three tags (JS + div + external CSS). The CSS can be inlined into the JS
with a config flag if you'd rather ship one file; I left it split to show the
file count honestly.

## External styleability (can host-page CSS reach the widget internals?)

| Artifact | Reach |
|---|---|
| React baseline | `.search-container` hook works. Everything else is `.sc-<hash>` styled-components classes — reachable but undocumented and re-hashed per build. Results dropdown is portaled to `<body>`, so host selectors scoped to the page region **miss it**. |
| Svelte CE, `shadow: "none"` | Same story: my semantic classes (`search-container`, `search-input`, `search-result`, …) are all reachable from host CSS. Svelte adds a `.svelte-<hash>` co-class (specificity 0,2,0) — a bare host selector loses a tie but wins with any extra specificity (`#main .search-result`). CSS lands in `<head>`, global. |
| Svelte CE, `shadow: "open"` (default) | Host CSS **cannot** reach in at all except via `::part()` / CSS custom properties you explicitly expose. Kills the external-styling story. Don't use it here. |
| Svelte mount-to-div / lean | Best case. Light DOM, semantic classes, results rendered **inline** in the container (I dropped the body-portal — an absolutely-positioned child covers the same visual case), so page-region-scoped host selectors work. |

Net: **all three keep the same one real hook the React version has** (`search-container`)
plus I added `search-input` / `search-result` / `result-match` / `result-source` /
`result-timestamp` for free in the Svelte port. Shadow DOM is the only thing that
would break styling, and `shadow: "none"` opts out of it cleanly while keeping
the custom-element ergonomics.

## Stability over ~3 years untouched

**React baseline — moving parts**
- Ships **react + react-dom 19**. The compiled bundle is frozen, so it won't
  "rot" on its own — but it's 240 KB gz of framework you're now maintaining a
  copy of. Security fixes to React don't reach a pinned inlined copy.
- Apollo Client 4 + `graphql` 17: `graphql` 17 is very new (still stabilising).
  Again frozen once built, but a large surface.
- `createRoot` / StrictMode / `useId` — all stable API, low risk.
- Real risk is **not** browser breakage; it's that nobody will want to rebuild
  this 800 KB artifact, so it ossifies at React 19 forever.

**Svelte CE / div — moving parts**
- Svelte 5 compiles to imperative DOM calls (`document.createElement`,
  `.append`, `.textContent`, event listeners) + a ~10 KB reactive runtime
  (`$state` proxies, effect scheduling). No vDOM, no reconciler. The DOM APIs it
  emits have been stable since ~2015 and are not going anywhere.
- The `graphql-request` → `graphql` parser is the least durable dependency in
  these two: `graphql` 16 is mature, but it's 45 KB of code you don't need.
- Custom-element registration (`customElements.define`, `attachShadow` code path
  present but unused with `shadow:none`) — Web Components v1 is a settled
  standard, broad support, no deprecation on the horizon.
- Svelte 5's `mount()` API is new (replaced `new Component()` from Svelte 4). If
  you never rebuild, irrelevant. If you rebuild in 3 years on Svelte 6, expect a
  small entry-point migration (same magnitude as React 18→19's `createRoot`).

**Svelte lean — moving parts**
- Just the Svelte 5 runtime + `fetch` + `URLSearchParams`. Both web APIs are
  ancient and stable. This is as close to "vanilla JS that doesn't rot" as any
  of these get. The only thing that could break it is a Svelte-runtime
  assumption about the JS engine, which is the same risk every bundle here has.

**Verdict on the durability axis:** the Svelte claim mostly holds, but the
*framework runtime* isn't the deciding factor — a frozen React bundle doesn't rot
either. The deciding factors are (1) **size** — a 13–28 KB artifact is one
someone will actually be willing to rebuild and re-audit in 3 years; an 800 KB
one gets left alone out of fear — and (2) **fewer dependencies with churn**:
Svelte-lean has two web APIs and a runtime; React-baseline has React 19 + Apollo
4 + graphql 17, three separately-versioning ecosystems.

## Cut corners (deliberate)

- **No runtime DOM test.** Builds compile clean and use standard Svelte 5
  `mount` / CE registration, but I did not mount them in a real browser or jsdom
  (would've needed another dep install). Risk: low, but the CE `shadow:none` +
  nested-component style-injection path is the one I'd smoke-test first.
- **React baseline build is my config, not theirs.** The package has no build
  script. I wrote a minimal lib-mode `vite build`. Their real production config
  might differ (code-splitting, externalized React, etc.) — externalizing
  react/react-dom would cut ~130 KB raw but then the host page must provide them.
- **Svelte port is behaviour-approximate.** I dropped the `getBoundingClientRect`
  + body-portal positioning dance and render results inline. Same visual result
  for the common case, simpler, better for styleability. If the dropdown must
  escape an `overflow:hidden` ancestor, that portal logic comes back (~1 KB).
- **No debounce/transition** on input in either port — matches the React
  source's `// FIXME: Should be a deferred value or transition`.
- **GraphQL types not regenerated** for Svelte — reused the existing
  `search.gql.ts` shape by hand; the Svelte version is plain JS, no typed
  document node.
- Didn't measure real gzip-over-the-wire (brotli would be ~15% smaller across
  the board; ratios hold).
- `query-string` kept as a dep in the non-lean Svelte builds but tree-shakes out
  (I used `URLSearchParams`).
