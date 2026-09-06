# React → Svelte port: working brief

Shared context for the two-agent port team. Source of truth for scope, order,
and constraints. Update the checklist as work lands.

## Goal

Reimplement `packages/editor/` (React 19 + TanStack Router + Apollo +
styled-components + React Spectrum S2) as `packages/editor-svelte/`
(Svelte 5 runes + SvelteKit + urql + scoped styles + Spectrum Web Components).

## Team

- **Sonnet (`porter`)** — writes code. Commits in small hunks. After each
  commit, asks Opus to review before continuing.
- **Opus (`reviewer`)** — plans, reviews each commit, does not write feature code.

## Build order: vertical slice first

The slice is `/transcript/[interviewNumber]` — the 268-line route plus the
Lexical editor it hosts. Port only the styles, queries and helpers that route
actually needs. Rationale: Spike 3 already de-risked the Lexical decorator seam,
but flagged the `createModel` singletons as the real pre-port task, and only
building this route exercises that decoupling.

Order within the slice:

1. Tooling fixes (see below) — one commit, no feature code.
2. `playhead` singleton → `.svelte.ts` runes module.
3. Lexical framework-neutral core: `commands.ts`, `shared.ts`, node classes.
4. Svelte decorator seam (`registerSvelteDecorator`), per Spike 3.
5. Extensions, one or two at a time.
6. The route itself: loader, video sync, editor host.
7. Remaining routes (`/`, `/transcript/`, `/transcript/create`), theme, search
   reconciliation.

## Tooling: validated state

Verified working:

- Vite dev server on `:2020` (`yarn dev`). Both `/` and `/search` return 200.
- GraphQL API live at `https://api.auohp.localhost/graphql` (self-signed cert).
- `graphql-codegen` run against the live API; output in `src/__generated__/`
  and `src/lib/__generated__/`. Needed
  `CODEGEN_GRAPHQL_ENDPOINT=https://api.auohp.localhost/graphql` and
  `NODE_TLS_REJECT_UNAUTHORIZED=0`.
- `oxlint` parses `.svelte` script blocks when given a directory
  (`oxlint src/routes/`), not a `*.svelte` glob.
- `stylelint` parses `<style>` blocks via `postcss-html`.

Known gaps, accepted by the user:

- **`svelte-check` is out of scope.** It demands TS 6 and TS 7 side by side;
  the package has TS 7 only. There is therefore **no type checking of template
  expressions**. Compensate with `vite build`, the Svelte MCP `svelte-autofixer`,
  and careful review.
- Svelte-aware ESLint is not set up. `oxlint` covers script blocks only.

Tooling commit (#1) should fix:

- Rune globals in `oxlint.config.ts` (`$props`, `$derived`, `$state`, `$effect`,
  `$bindable`, `$inspect`, `$host`) — currently every component trips `no-undef`.
- `import-x/no-internal-modules` `allow` list still names React-era packages
  (`@apollo/client/**`, `@react-spectrum/s2/**`) and rejects `vitest/config`
  and `@sveltejs/kit/vite`.
- `import-x/resolver` points at `packages/editor/tsconfig.json` — wrong package.
- `codegen.ts` default endpoint is `http://…`; the API is `https://…`.
- `vitest.config.ts` loads `vitest-browser-react` as a setup file and its
  comments describe the React JSX transform. Swap to `vitest-browser-svelte`.
- `stylelint.config.js` routes `.ts`/`.tsx` through `postcss-styled-syntax`
  (dead weight); `:global` trips `selector-pseudo-class-no-unknown`.

## Constraints

- **Testing is out of scope.** Write exactly one example component test and one
  example unit test. Nothing more.
- **Inline comments: terse and informative. No editorialising.** The React
  source is heavily commented in an essayistic register; do not carry that
  across. Where a comment explains a genuine non-obvious mechanism, keep it
  short.
- Use `--` and `---` for dashes in comments, not Unicode en/em dashes.
- No all-caps emphasis in comments.
- Do not fix linter style complaints (indentation, wrapping, spacing). Fix real
  type errors and bugs.
- Four-space indent, double quotes, trailing commas — match existing files.
- The Svelte MCP server and `svelte-code-writer` / `svelte-core-bestpractices`
  skills are available and should be used when writing `.svelte` /
  `.svelte.ts` files.
- Run `svelte-autofixer` on Svelte code before committing it.
- `modern-web-practices` plugin is a **last pass**, after code is locked. Not now.

## Test data

Interviews available on the live API:

| number | uid          | interviewee        |
|--------|--------------|--------------------|
| 23     | 72FXNWWjDX   | Emily Nahmason     |
| 26     | 1J2xt9rj3j   | Iris Long          |
| 28     | Msj1eZekxH   | Richard Deagle     |
| 43     | xNYgEMPn1G   | Maxine Wolfe       |
| 108    | qXqde1w3Nv   | Avram Finkelstein  |

## What evaporates in the port

Framework tax with no Svelte equivalent — do not port these:

- `router.tsx`, `routeTree.gen.ts` — SvelteKit filesystem routing.
- `styles/global/styled-components-registry.tsx` (114 lines) — solves React SSR
  style insertion; Svelte's scoped `<style>` has no such problem.
- `@preact/signals-react` + its Babel transform, `@rolldown/plugin-babel`,
  `useSignalEffect`, `.value` reads — runes.
- `ClientOnly` wrapper around the editor — becomes an `onMount` / `{#if browser}`
  guard, for the same underlying reason (Lexical is uncontrolled, no SSR DOM).
- Font `.tsx` files (784 lines of `createGlobalStyle`) — plain CSS.
- `useMemo` on the extension keyed by `interviewUid` — SvelteKit's `{#key}` or
  component identity.

## Known defects in the source — do not replicate

- `routes/index.tsx` and `routes/transcript/index.tsx` are near-duplicates
  (90 / 95 lines), both defining `LIST_INTERVIEWS_QUERY`, both with a dead
  `LinkComponent` styled-component. Collapse to one.
- `routes/index.tsx` imports `./transcript/search/route`, a path that does not
  exist (search lives at `src/routes/search/route.tsx`).
- `routes/search/results/route.tsx` had a stray `A;` statement; already dropped
  in the spike port.

## Spike inheritance

`src/lib/` and `src/routes/search/` are spike code from `spike-svelte-search`,
already adjusted by the user (tabs→spaces, `<sp-theme>` replaced with direct
CSS token imports, preflight removed because the web components are not robust
to it). Notable decisions carried in that code:

- `search.svelte.ts` — `$state` object replacing the `createModel` singleton.
- `urql.ts` — `preferGetMethod: false` is load-bearing; the API serves GraphiQL
  on `GET /graphql` and only answers queries on POST. urql's default result
  handling already matches Apollo's `errorPolicy: "all"`.
- `search/+layout.svelte` + `search/+page.svelte` — nested layout replacing
  TanStack's `mask: { to: "/search" }`. Results were never URL-restorable, so
  this is behaviourally equivalent.
- `lib/graphql.ts` — hand-written types, now superseded by codegen output in
  `src/lib/__generated__/graphql.gql.ts`. Reconcile.

Lexical spike code was **not** copied over. It lives on branch
`spike-svelte-lexical` (SHA `e33edce`, addendum `8bb9b63`) with
`LEXICAL-SPIKE-NOTES.md`. Read it before porting the editor.

## Pre-port task flagged by Spike 3

`createModel` module singletons (`playhead`, `SearchQuery`) couple editor
instances. Decoupling them is framework-independent and should happen early.
Extension state via `namedSignals` is already per-editor and fine.

**Correction (they are not the same case).** Spike 3 named both together; that
conflates two different situations, and they get opposite treatments:

- `searchQuery` **stays a shared singleton.** That is a settled design decision
  (sole source of truth, one commit per search). One search box, one result set,
  app-wide. Already ported as `lib/search.svelte.ts`. Do not make it
  per-instance.
- `playhead` **becomes per-instance.** Its module scope is an artifact of the
  Slate-vs-Lexical bake-off -- it was lifted so both editors shared identical
  video-sync machinery for an honest comparison. That comparison is over, and
  the Slate route is gone. One video per interview route means per-instance is
  the correct model; module scope only ever worked because two editors are
  never on screen at once.

Consumers of `playhead` to rework: `lexical/nodes.tsx:128`,
`lexical/extensions.tsx:198,329,339`, and the route's video sync.

## Commit protocol

Small, focused commits. No broken intermediates. After each commit the porter
asks the reviewer to check the work before continuing. Commit messages end with:

```
Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01GWi9W42XvWNggZG3VAHyaL
```

## Session pause --- 2026-09-05, resume here

Team stopped mid-step-2. State of the world:

**Committed:** `eef8c65` (tooling fixes) only.

**Uncommitted, ready to land as three commits** (boundaries already agreed):

- **2a** --- the `.gitignore` anchoring fix alone (`lib/` -> `/lib/`, `lib64/` ->
  `/lib64/`). Repo-root change, revertable on its own.
- **2b** --- the previously-invisible files: `src/lib/{search.svelte.ts,urql.ts,
  graphql.ts,index.ts}`, `src/lib/__generated__/`, `src/__generated__/`, plus
  `MIGRATION.md`, `PLAN.md` and the uncommitted `oxlint.config.ts` resolver
  change. Commit as-is, no cleanup --- the diff should read purely as "these
  become tracked". Precedent checked: `packages/editor` tracks 6 `__generated__`
  files, so codegen output is committed in this repo.
  **`src/lib/assets/` cannot be staged** --- see the blocker below. Note it in
  the commit message.
- **2c** --- `src/lib/playhead.svelte.ts` alone. Already written and verified
  (`svelte-autofixer` clean, compiles under `vite build`). Factory only, no
  default instance, per §3.1. Needs a review round; 2a and 2b do not.

**Blocked on a user decision:** `.git/info/exclude` lines 9-10 contain a bare
`assets` and `**/assets/**`. Both must go for either to matter. Consequences:

- `packages/editor-svelte/src/lib/assets/` is invisible.
- `packages/editor/src/styles/assets/` is invisible --- **zero** tracked files.
  Those are the font sources step 7 ports.
- Repo-wide, only two paths under any `assets/` are tracked, both `.gitkeep`
  placeholders whose contents were then swallowed.
- The file is local-only and never travels, so a fresh clone behaves differently.

Until this is resolved, nothing under an `assets/` directory can be committed
anywhere in the repo. Do not fight it with negation rules or directory renames.

**Open defect, logged in PLAN.md §7:** `no-internal-modules` has never run.
oxlint does not supply `eslint-plugin-import-x` with a resolver, so the
resolver-dependent `allow` form is inert; the `forbid` form works and is the
agreed fix. Same defect exists in `packages/editor`. Matters at steps 3-5.
Instrument caveat: `oxlint --print-config` omits jsPlugin rules wholesale and
`--deny` is silent even for bogus rule names --- neither can tell a disabled
rule from a passing one.

**Standing cleanups (PLAN.md §7):** drop `@types/react`; widen the vitest
`include` glob (currently `test/**/*.{test,spec}.{ts,tsx}`, cannot match a
component test). Note `*.svelte.ts` is a **reserved** filename pattern ---
`vite-plugin-svelte` compiles anything matching `/^[^?#]+\.svelte\.[jt]s(?:[?#]|$)/`
as a rune module, so test files must not be named that way.

**Verification discipline that this session established.** Five defects so far
all failed by producing the appearance of success. Plain `git status` does not
list ignored files; three of us reported a clean tree and were wrong. Use
`git status --short --ignored <path>` and `git check-ignore -v <file>`. For a
linter, feed it a known-bad input and confirm it rejects --- silence is not
evidence.

## Checklist

- [ ] 1. Tooling fixes
- [ ] 2. `playhead` → runes
- [ ] 3. Lexical neutral core (commands, shared, nodes)
- [ ] 4. Svelte decorator seam
- [ ] 5. Extensions
- [ ] 6. `/transcript/[interviewNumber]` route
- [ ] 7. Remaining routes + theme + search reconciliation
- [ ] 8. Example tests (one component, one unit)
- [ ] 9. `modern-web-practices` last pass
