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
- ~~`routes/index.tsx` imports `./transcript/search/route`, a path that does
  not exist~~ --- **corrected, step 7 session.** This was a documentation
  error, not a source defect: `routes/index.tsx:8` imports `./search/route`
  and `routes/transcript/index.tsx:8` imports `../search/route`, both of
  which correctly resolve to `src/routes/search/route.tsx`. Did not affect
  the port either way (the collapsed `/` route links to `/search` directly).
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

**Corrected, step 5 session:** `no-internal-modules` is not inert. The earlier
conclusion (this file and PLAN.md §7) was wrong about the mechanism and the
outcome. `import-x/resolver`'s `typescript.project` path
(`packages/editor-svelte/tsconfig.json`) is monorepo-root-relative; run oxlint
from a cwd where that relative path does not resolve (a scratch worktree, for
instance) and the resolver silently fails, the rule fails open, and it looks
inert. Run from the actual repo --- the package directory or the repo root,
both tested --- and the resolver works and the rule fires for real.

Currently reports three errors in `src/lib/editor/`: `StatementNode.ts` ->
`persistence/synthetic-uid`, `PersistenceExtension.ts` -> `statement/
StatementNode`, `TagSplitBoundaryExtension.ts` -> `statement/
StatementExtension`. All three are accepted, intentional cross-feature seams
from steps 3-5 (PLAN.md sec 1.2), not a backlog to clear --- the rule is
correctly identifying them as reaching past a `dir/index.ts` barrel, which is
exactly right, except `src/lib/editor/`'s feature directories deliberately
have no barrels (71c3e95's log entry above). The rule is policing a convention
this package doesn't follow, which is why it flags correct code.

No lint script, CI workflow, or hook runs oxlint in this package, so nothing
has been silently red; these errors have only ever surfaced when a person runs
oxlint by hand. `forbid` (pure pattern matching, no resolver needed) remains
the right eventual fix, deferred until the module graph settles after
search-interview (step 5 commit D) lands --- not indefinitely, and not because
the rule doesn't work. Instrument caveat, still true: `oxlint --print-config`
omits jsPlugin rules wholesale and `--deny` is silent even for bogus rule
names --- neither can tell a disabled rule from a passing one; the only
reliable check is a known-bad input from the actual repo location.

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
- [x] 4. Svelte decorator seam --- **closed.** `registerSvelteDecorator`
      ported (`d0d78c5`), `TagChip.svelte` + `TagChipExtension.ts` wired as
      its first consumer, four-gesture component test added (`a743dba`).
      Editor reference goes through decorator props, not context (`mount()`
      doesn't cross Svelte context boundaries) --- no module singleton, same
      class of mistake as step 3's playhead setter.

      **Tooling fix (`c211828`):** the actual defect was a missing `test`
      script in `package.json` --- there wasn't one, so `vitest` had never
      been run against this package at all. The `.tsx` → `.ts` glob narrowing
      was also made per PLAN.md's settled decision, but was not itself the
      reason tests weren't collecting; the earlier session-log entry (this
      file, superseded) stated the glob as the cause and that was wrong.

      **The sweep's justification, corrected.** The dispatch brief and
      PLAN.md both state `registerUpdateListener`'s sweep is required because
      root detach/reattach "produces no mutation record at all." Traced
      against the actual lexical 0.49.0 reconciler source and measured
      directly (porter + reviewer, this session): the **detach** half
      (`setRootElement(null)`) is genuinely silent --- `resetEditor` nulls
      the mutation observer before `$commitPendingUpdates` runs, so
      `$reconcileRoot` never executes. But the **reattach** half fires
      `FULL_RECONCILE`, which re-announces every live node as `"created"`
      regardless of whether it changed -- so the mutation listener alone
      already recovers once reattach happens. No test constructed against
      this Lexical version shows the sweep changing an outcome. It stays in
      (cheap, and PLAN.md still calls it non-negotiable), but is now
      documented as insurance against an unexercised path rather than a
      demonstrated fix -- see `svelte-decorator.ts`'s inline comment
      and `test/decorator-seam.test.ts`'s module comment for what was
      actually measured versus assumed. Same correction applies to PLAN.md
      §4's build-order entry for this step.

      **The `"updated"`-mutation test, made to discriminate (`f28ace8`).**
      The original test asserted a mount count `ensure()` never increments
      for an already-known key, and looked at a DOM element already
      correctly parented from initial mount -- it passed whether or not the
      mutation listener's `"updated"` branch did anything. Fixed with a
      `forceRebuild` flag on the test fixture node so `updateDOM()` can
      genuinely report a rebuild. Mutation-tested both directions: red with
      the sweep disabled and the `"updated"` branch broken; green with the
      sweep intact and only that branch broken, because the sweep's next
      pass repairs it first -- the sweep is a strict superset of the
      mutation listener's `"created"`/`"updated"` handling in this Lexical
      version. The test proves the seam as a whole survives a rebuild; it
      cannot isolate the listener's `"updated"` branch from the sweep. Said
      so directly in the test rather than implying independent coverage of
      both mechanisms.
- [x] 5. Extensions --- **closed.** All five extensions ported: persistence,
      tag-split-boundary, latency, statement-seek, update-timestamp (prior
      commits), and search-interview (commit D, this session). Reviewer
      verified search-interview line-by-line against
      `packages/editor/src/lexical/extensions.tsx:769-1312`; no correctness
      drift, including the four `peek()` -> `untrack()` sites and the
      jump-to-first-hit regression class (does not reproduce --- query-change
      and highlight-pass effects have disjoint triggers, matching the
      source's deliberate asymmetry). `findMatchRanges` unit test added
      (`test/match-ranges.test.ts`, PLAN.md's designated step-8 target,
      pulled forward since it was small/pure and already earmarked) ---
      6 cases, all pass alongside the existing 8. One dead-code note from
      that test: the `start < lastEnd` overlap guard in `match-ranges.ts:59`
      cannot currently be exercised, because `matchAll` on a fixed-width
      literal+`\b` pattern always resumes past the previous match; written as
      an invariant check instead of a fabricated overlap, so it still catches
      a regression if the pattern ever grows a variable-width piece. The
      `createSearchOutput` behavioral test (query/data/clamp/re-search-bypass)
      was scoped by the reviewer but deferred --- judgment call against the
      two-test budget PLAN.md sets for the whole port, not a gap being
      hidden. `no-internal-modules`'s `forbid`-migration (deferred pending
      this module graph, PLAN.md sec 1.2/7) can now proceed whenever picked
      up.
- [x] 6. `/transcript/[interviewNumber]` route --- **closed.** Ported
      `packages/editor/src/routes/transcript/$interviewNumber.tsx` (268
      lines) to `+page.ts` (urql load, `HEADER_QUERY` + `TRANSCRIPT_QUERY`)
      and `+page.svelte`. `defineAuohpEditorExtension` (new,
      `src/lib/editor/editor.ts`) is the composition root wiring all six
      extensions from step 5 plus `HistoryExtension`/`RichTextExtension`,
      dependency order matching the source's `KEY_ENTER_COMMAND` priority
      race. `{#if browser}` replaces `ClientOnly`, `{#key interviewUid}`
      replaces the source's `useMemo`. The playhead is created INSIDE the
      `{#key}` block per PLAN.md's explicit instruction --- reviewer
      confirmed this fixes the source's stale-`seek`-on-interview-switch
      race as a side effect of correct lifetime scoping, not just style.
      `EditorHost.svelte` (new, no source equivalent) bridges
      `buildEditorFromExtensions`/`setRootElement`/`.dispose()` where the
      source used `LexicalExtensionComposer`, and houses the two
      video<->playhead sync effects. Two fields dropped versus the source's
      query docs (`wroteEmbedding`, top-level `health`) --- verified dead in
      the source itself, not scope creep. No latency meter UI: the ported
      `LatencyExtension` has no `Component` output, so there is nothing to
      place; confirmed not a silently-dropped feature.
- [x] 7. Remaining routes + theme + search reconciliation --- **closed**,
      with one explicit carve-out. Collapsed `routes/index.tsx` +
      `routes/transcript/index.tsx` (confirmed near-duplicates: identical
      `LIST_INTERVIEWS_QUERY`, identical dead `LinkComponent`) into one `/`
      route, replacing the spike's `goto("/search")` stub. Theme: zero
      tokens ported --- Spectrum's CSS custom properties carry no
      `@font-face` rules, and the already-committed layout's font-family
      fallback stack already covers everything the built slice reads;
      confirmed via grep that no route in transcript/search/list references
      a custom-font class. `lib/graphql.ts`: kept the query string (real
      codegen input), deleted hand-duplicated type literals, re-exported
      from `__generated__/graphql.gql.ts` instead --- `yarn codegen` diffed
      clean, proving the hand types were accurate rather than stale.

      **Carve-out, not silently absorbed:** `/transcript/create` (source:
      `routes/transcript/create.tsx`, a health-check page) has no
      editor-svelte route yet. `routes/transcript/index.tsx`'s "New/Create"
      link pointed there; porting the link with no destination would be
      dead, so it was dropped rather than faked. Still open --- pick up
      whenever that route gets built, not implicitly covered by this
      checkbox.
- [ ] 8. Example tests (one component, one unit)
- [ ] 9. `modern-web-practices` last pass

      **Carve-outs from the Opus code-quality audit (`.audit-opus.md`),
      logged rather than silently absorbed by a green checkbox:**

      - Finding 4: `TagSplitBoundaryExtension.ts:43-46` --- pressing Enter
        with the caret inside a tag chip silently no-ops (`preventDefault`,
        no split, no feedback). Faithful to the source, not a port defect,
        but the extension's own header comment already calls this "a real
        open question, not a nicety," and it is the only user gesture in the
        tree that goes nowhere. Open until someone decides what pressing
        Enter there should actually do.
      - Finding 7: `SearchInterviewExtension.ts:322` reads `interviewUid` off
        `PersistenceExtension`'s output rather than taking it directly, so
        search structurally depends on the write path to know what it's
        searching. Fails silently: `interviewUid` defaults to `""`
        (`PersistenceExtension.ts:73`), so a missing wiring produces
        unscoped results, not an error. `editor.ts:39` already threads
        `interviewUid` in as a top-level option, so passing it to both
        extensions independently would remove the coupling for the cost of
        one duplicated wiring line. Deferred at step 6, still deferred ---
        this entry is what keeps it from being silently dropped a second
        time.
