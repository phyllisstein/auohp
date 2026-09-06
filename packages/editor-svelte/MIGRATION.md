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

## Session log --- 2026-09-05/06, resume here

**Resolved, no longer a blocker:** the `.git/info/exclude` bare `assets` /
`**/assets/**` lines and `.gitignore`'s unanchored `lib/` are both gone.
`packages/editor-svelte/.gitignore` carries `!lib/` / `!assets/` negations
(landed in `a5f49a4`, predates this session's later work). Assets and
`src/lib/` commit normally now --- verified via `git check-ignore -v`.

**Committed so far, tip `431fc9e` on `svelte-editor`:**

- `eef8c65` --- tooling fixes (pre-existing).
- `e90b985` --- previously-invisible spike files (`src/lib/{search.svelte.ts,
  urql.ts,graphql.ts,index.ts}`, `__generated__/` x2), `MIGRATION.md`, `PLAN.md`.
- `13179c9` --- step 2, `playhead.svelte.ts` (per-instance factory, reviewed).
- `71c3e95` --- step 3 part 1: command tokens split to their owning feature
  dirs (no barrel file), `formatTimestamp` -> `statement/timestamps.ts`,
  `SYNTHETIC_UID_MARKER` -> `persistence/synthetic-uid.ts`. Approved.
- `fe6f1d8` --- `StatementNode.ts` port. **Superseded** by `3d93f07` (see below);
  kept in history rather than rewritten.
- `3f5106d` --- `TagChipNode.ts` + `SearchResultNode.ts`. Approved, no shared
  base class (PLAN.md §6 non-goal), source's duplicated-header defect not
  carried across.
- `a8968c6` --- first attempt at fixing `fe6f1d8`'s playhead seam. **Superseded**
  by `3d93f07`: used a type-cast stand-in extension object
  (`{name: "..."} as unknown as LexicalExtension<...>`) that would throw in
  *every* case once a real extension existed, because `@lexical/extension`
  resolves dependencies by name **and then asserts reference identity**
  (`LexicalBuilder`'s `getExtensionRep`, throws on mismatch). A stand-in can
  never satisfy that check --- there is no fix short of a real shared object.
- `3d93f07` --- real fix: `StatementExtension.ts`, a genuine minimal
  `defineExtension` (name + `nodes: () => [StatementNode]` + config + build),
  module-level singleton import, no cast. This part is solid and unchanged
  since.
- `6a4e1a2` --- trivial all-caps comment nit in `TagChipNode.ts`. Approved.
- `31cb87d` --- struck stale "assets blocked" wording from this file / PLAN.md.
- `431fc9e` --- fixed `3d93f07`'s remaining issue: its config default was a
  throwaway `createPlayhead()` instance, which silently reintroduces the exact
  corruption failure (`startTime: 0` written on split) if a route ever forgets
  to override it via `configExtension` --- same risk as the rejected `?? 0`
  fallback, just relocated to a wiring omission instead of an ordering bug.
  Fixed: `playhead: Playhead | null`, default `null`, `build()` throws if still
  null. Refusal happens before the editor finishes construction, and `build`'s
  return value **is** `.output`, so nothing can reach the read site without
  passing the check. Also resolved a live question about whether
  `createPlayhead()` (`$state()`) is even legal to call at module-evaluation
  time in a plain `.ts` file rather than `.svelte.ts` --- moot now, the call
  is gone.

**Reviewer confirmed pending final pass, expected to close step 3 clean** as
of the last exchange in this session. If step 3 isn't marked done below and
you're resuming cold, check with `git log --oneline` against the tip above and
`reviewer`'s last message before assuming anything is still open.

**Design principle worth keeping visible for steps 4-6:** a `null` config
default is only a safe pattern when every read site is guarded and the `null`
is a real, intended mode (see `PersistenceConfig`'s `null` executors, disabled
persistence, always read via `?.`). A `null` that must never actually reach a
read site is a different thing --- a sentinel, not a value --- and belongs
behind a thrown guard (in `build`, ideally, where refusal prevents construction
altogether) rather than an unguarded optional field. Don't let the shared
`null` literal make these look like the same pattern; ports of extension
configs in steps 5-6 should ask which one they are.

**`@lexical/extension` mechanism worth remembering for steps 4-5:** extensions
are identified by **object identity**, not name or shape --- name is only an
index into `LexicalBuilder`'s map, the map entry's `extension` field must be
`===` the object passed to `$getExtensionDependency`. Any extension referenced
from node code (or elsewhere) must be a real, module-level singleton, exported
once and imported by reference. A structurally-identical re-creation, or a
type-only stand-in, throws --- this is nominal typing enforced at runtime.

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

**Verification discipline established this session.** Multiple defects in a
row failed by producing the appearance of success: a green `vite build` on
`StatementNode.ts` proved nothing while nothing imported it yet, and the
`as unknown as` cast in `a8968c6` suppressed the only error TypeScript could
have raised. Plain `git status` does not list ignored files either. Use
`git status --short --ignored <path>` and `git check-ignore -v <file>` for
gitignore questions; for extension/dependency-graph code, trace the actual
library source (`node_modules/@lexical/extension/dist/*.d.ts` and the
`.dev.mjs` implementation) rather than trusting a plausible-sounding claim
about runtime behavior. For a linter, feed it a known-bad input and confirm it
rejects --- silence is not evidence.

**Dev/API environment (2026-09-06):** Vite dev server on `:2020`, plain
`http://api.auohp.localhost/graphql` reachable (no TLS workaround needed ---
Docker's back up, self-signed-cert `https://` fallback no longer required
day-to-day, though `codegen.ts`'s default stays `https://`).

## Checklist

- [x] 1. Tooling fixes
- [x] 2. `playhead` → runes
- [x] 3. Lexical neutral core (commands, shared, nodes) --- **closed, reviewer
      approved the full set** (71c3e95, 3f5106d, 3d93f07, 6a4e1a2, 31cb87d,
      431fc9e; fe6f1d8 and a8968c6 superseded by 3d93f07). Deferred to steps
      5-6: `shared.ts`'s Apollo-hook-derived type aliases (`TranscriptStatements`,
      `EditStatementFn`, etc.), which belong with the urql operation documents
      that replace them. Carry-forwards logged in PLAN.md §7: `Temporal` has no
      polyfill and will fail at runtime the first time a statement renders
      (step 5/6); two leftover all-caps words in `StatementNode.ts:144,219`.
- [ ] 4. Svelte decorator seam --- **dispatched to porter, in progress.**
      PLAN.md §5's highest-ranked risk. Two non-negotiable invariants: handle
      `"updated"` mutations (not just created/destroyed), and sweep on
      `registerUpdateListener` re-parenting when `getElementByKey` returns a
      different host (the correctness-critical one, silent-failure-prone --
      catches root detach/reattach with zero mutation records). Editor
      reference goes through decorator props, not context (`mount()` doesn't
      cross Svelte context boundaries) --- no module singleton, same class of
      mistake as step 3's playhead setter. Test budget's component half spends
      here: the four-gesture table from the spike notes. `vitest.config.ts`'s
      `include` must be fixed as part of this step --- widen `test/**/*.{test,
      spec}.{ts,tsx}` to drop `.tsx` only, do NOT reach into `src/` (the
      `*.svelte.ts` reserved-pattern hazard plus no benefit at this test
      budget size); getting this wrong means vitest silently collects 0 tests
      and exits green. Read `LEXICAL-SPIKE-NOTES.md` on branch
      `spike-svelte-lexical` (SHA `e33edce`, addendum `8bb9b63`) before
      starting --- port `registerSvelteDecorator` essentially verbatim.
- [ ] 5. Extensions
- [ ] 6. `/transcript/[interviewNumber]` route
- [ ] 7. Remaining routes + theme + search reconciliation
- [ ] 8. Example tests (one component, one unit)
- [ ] 9. `modern-web-practices` last pass
