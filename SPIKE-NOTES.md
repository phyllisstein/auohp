# SvelteKit port spike — editor search feature

Scope: ported `packages/editor/src/routes/search/` (route + `results/` child +
`-search-signal.ts` + its generated GraphQL types) to a fresh SvelteKit app at
`spike-svelte-search/`. Lexical, transcript routes, everything else: untouched.

`npm run dev` in `spike-svelte-search/` serves `/search`. Verified end-to-end in
a real browser against the **live GraphQL API**: type "activism" → click Search →
urql POST → real interview data renders in `sp-card`s. Error path also verified
(network failure → `CombinedError` → alert branch).

## What got ported

| Original | Port |
|---|---|
| `-search-signal.ts` (`createModel` singleton) | `src/lib/search.svelte.ts` — one `$state({...})` object |
| `route.tsx` `SearchPage` | `src/routes/search/+layout.svelte` — search bar chrome + `runSearch` |
| `results/route.tsx` `ResultsPage` | `src/routes/search/+page.svelte` — the results view |
| `__generated__/index.gql.ts` + inline `gql` | `src/lib/graphql.ts` (hand-transcribed, see stub note) |
| Apollo client + tanstack integration | `src/lib/urql.ts` — `@urql/svelte` `Client` |

## What was stubbed / cut

- **GraphQL codegen not run.** `src/lib/graphql.ts` is hand-transcribed from the
  committed `index.gql.ts` + the operation string. Real port: `graphql-codegen`
  with `near-operation-file-preset` is framework-neutral — copy `codegen.ts`,
  point `documents` at the new `src/`, `npm run codegen`. Needs the API up for
  introspection or a local `schema.graphql` (editor has one committed).
- **SSR disabled** (`src/routes/+layout.ts` → `export const ssr = false`). SWC
  are browser-only custom elements; the original `results/route.tsx` already had
  `ssr: false`. Doing it app-wide is the lazy spike choice — real port would use
  `onMount` guards or `@lit-labs/ssr`, or just keep search CSR.
- **Spectrum: Gen 1 is the deliberate target now** (see follow-up below).
  `@spectrum-web-components/*@1.x`, `sp-theme system="spectrum"`, Gen-1
  component APIs and `--spectrum-*` Gen-1 CSS props. Nothing S2. Everything
  previously stubbed for "S2 not ready" now uses the real Gen-1 component
  (`sp-card` for results, `sp-icon-search` on the button).
- **`sp-search` submit form.** The `<sp-search>` element wraps an internal
  `<form>` and shows its own Reset button; wired `oninput`/`onkeydown` to the
  shared state, didn't fight its form semantics.
- styled-components registry, the `style()` S2 macro, `@rolldown/plugin-babel`
  + signals transform: all deleted, no equivalent needed.
- Root `/` just redirects to `/search`.

## Per-leg friction

### State — frictionless (the standout)
`createModel` singleton + `signal()` + `.value` + the entire Babel-transform
apparatus → one `$state({...})` in a `.svelte.ts` file. `.value` reads become
plain property reads; `batch()` is unnecessary (Svelte batches synchronous
mutations per tick); the `@rolldown/plugin-babel` /
`@preact/signals-react-transform` / `@preact/eslint-plugin-signals` stack in
`vite.config.ts` evaporates entirely. `$derived` replaced the "compute `hits`
from `results`" line cleanly. This leg is a straight win.

### GraphQL — smooth, one semantic gotcha handled
`@urql/svelte` is a genuine 1:1 for the imperative-query shape. `useLazyQuery` →
`client.query(...).toPromise()`. **The load-bearing `errorPolicy: "all"` is
free**: urql's default already resolves (never rejects) on GraphQL errors, with
`{ data, error: CombinedError }` both populated — exactly Apollo AC4's `"all"`
behaviour. Wired explicitly: `searchQuery.error = result.error ?? null` next to
`searchQuery.results = result.data ?? null`. No per-op errorPolicy knob to
forget. Verified: a network failure surfaced as `CombinedError` "[Network]
Failed to fetch" through the error branch, didn't throw.
**Update — urql GET default bit for real.** The AUOHP API serves the GraphiQL
IDE HTML on `GET /graphql` and only answers queries on `POST`. urql defaults
queries to GET, so the first live request parsed the IDE's HTML as a response
and surfaced it as a `[Network]` error. One-line fix: `preferGetMethod: false`
on the `Client`. After that, live search against the real API returns real
interview data. So: still smooth, but this is a concrete gotcha to write down —
Apollo defaults to POST, urql doesn't.

### Router — mostly mechanical, one trick restructured
File routes → `+page.svelte` / `+layout.svelte`: trivial. The interesting bit is
the `navigate({ to: "/search/results", mask: { to: "/search" } })` trick —
mounting the results child while keeping the URL at `/search` because the
results live in an in-memory signal and aren't URL-restorable. SvelteKit has **no
URL-mask primitive**. Chosen equivalent: **nested layout** — `search/+layout.svelte`
is the search-bar chrome, `search/+page.svelte` is the results view that always
renders and reads shared `$state`. No navigation call at all, no `/search/results`
route. Behaviourally identical (results were transient either way) and simpler.
Shallow routing (`pushState` + `page.state`) was the alternative — it's the
closer analogue to "mount a child without a real URL" but adds ceremony for zero
gain here.

### Components — REVISED, Gen 1 as the deliberate target

First pass blamed friction on "Spectrum v2 is half-shipped." Redid the UI
targeting **SWC Gen 1** on purpose to isolate that. Findings:

**The Spectrum-generation confound was real but small.** The v1.x SWC packages
(`@spectrum-web-components/*@1.12`) ARE Gen 1 — the earlier mistake was setting
`system="spectrum-two"` (the S2 bridge) instead of `system="spectrum"`. With
that fixed: theme delivers, all components render, `--spectrum-gray-100` etc.
resolve. Everything stubbed for "S2 not ready" had a Gen-1 component sitting
right there: `sp-card` for results, `sp-icon-search` on the button. No stubs
left in the UI.

**Residual friction — and every remaining item is Svelte's, not Spectrum's:**

- **Svelte 5 sets some attributes as DOM *properties* on custom elements.**
  `<sp-theme system="spectrum">` → Svelte writes `el.system = "spectrum"` (a
  property, because `SpectrumElement` declares an accessor), NOT the `system`
  attribute. `sp-theme`'s own runtime validation reads `getAttribute('system')`
  and warns it's missing — even though the property is set and the theme works.
  `color`/`scale` happen to land as attributes. Worked around with a one-line
  `onMount(() => el.setAttribute('system', 'spectrum'))`. The `attr:` directive
  that would fix this cleanly isn't in Svelte 5.56 yet. **This is the single
  most Svelte-specific gotcha and it's generation-independent** — any Lit
  element with reflected attributes hits it.
- **Svelte 5 dropped `?attr` boolean syntax** — `?pending={x}` is a compile
  error; use `pending={x || undefined}`. Generation-independent.
- **Svelte a11y linter doesn't model custom elements as interactive** — 3
  warnings for `oninput`/`onclick`/`onkeydown` on `sp-*`. Noise, ignored per
  project convention. Generation-independent.
- **SSR of custom elements** — punted app-wide (`ssr = false`). Not exercised,
  so not scored, but it's the one place custom elements genuinely need care in
  SvelteKit and it's Svelte/SSR's problem, not Spectrum's.
- `sp-theme` fragment loading is per-token side-effect imports — fiddly but
  documented, and it's an SWC design choice, framework-neutral.
- A `sp-progress-circle` deprecation warning fires from *inside* `sp-button`'s
  pending shadow DOM — SWC-internal, not our code, not fixable without patching.

**Verdict: the component leg is now ~90% as smooth as state/GraphQL/router.**
Native custom-element binding (events, props, slots via `slot="..."`) is clean
and needs no shim. The residual 10% is the property-vs-attribute quirk — a real
Svelte behaviour that bites any web-component library, cost one `onMount` line
here, and would compound if the UI had many themed/reflected-attr elements.
Nothing left points at Spectrum's generation.

## Bottom line

**Pleasant? Yes — and it holds after removing the Spectrum-v2 confound.** State
and GraphQL legs are net simplifications (real code deleted, no new
indirection). Router leg mechanical once the mask trick is reframed as a layout.
Component leg is close behind: the friction that survives is Svelte's
property-vs-attribute handling of custom elements, not the design system.

**Cheap? Yes, and slightly cheaper than the first pass concluded.** The
component leg's cost is a handful of small, well-understood Svelte quirks
(`?attr`, prop-vs-attr, a11y lint) — each a one-liner — plus one real decision
to make about SSR of custom elements. A few hours for the whole slice.

**The one thing that fought me, revised:** not "Spectrum v2 is a moving target"
(that was a self-inflicted `system=` typo). It's that **Svelte 5 writes
custom-element attributes as properties when the element declares an accessor**,
and libraries like SWC that also validate the attribute will complain. One
`onMount` reflection line per element class fixes it; a future `attr:` directive
will fix it declaratively.
