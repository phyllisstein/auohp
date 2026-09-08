# Migration plan --- design level

Companion to `MIGRATION.md`. That file holds scope, tooling state and the
commit protocol; this one holds the module layout, the seams, the order, and
the risks. Design level only --- no signatures, no bodies.

## 1. Target module layout

The source `src/lexical/` is a junk drawer by technical kind: one 2117-line
`extensions.tsx` holding eleven extensions plus six React components, one
622-line `nodes.tsx` holding three unrelated node families, one `commands.ts`
holding three unrelated commands. Nothing in that split tells you which pieces
belong together. The target is feature-scoped: a directory per capability,
owning its own nodes, commands, extension, state and components.

```
src/lib/
    editor/                 -- the framework-neutral core plus the Svelte seam
        statement/          -- StatementNode, its extension, seek, timestamps
        tag-chip/           -- TagChipNode, insert command, chip component
        search-interview/   -- in-editor find/replace: node, extension, driver, bar
        latency/            -- instrumentation
        persistence/        -- the write path
        svelte-decorator.svelte.ts  -- the seam (one file, shared)
        editor.ts           -- defineAuohpEditorExtension: the composition root
    playhead.svelte.ts      -- runes port of the createModel singleton
    urql.ts, search.svelte.ts, __generated__/   -- existing spike code
```

Each feature directory owns:

- its node class(es) --- plain `.ts`, no framework
- its command tokens --- co-located with the extension that handles them
- its `defineExtension` call
- its Svelte components, if any
- its styles

The rule that makes this work: **a command token lives with the extension that
registers a handler for it, not in a shared `commands.ts`.** All three current
tokens are single-consumer, so all three move.

### What crosses feature boundaries

Only three things, and each is a deliberate seam:

1. `playhead` --- read by `statement` (seek writes it, timestamp reads it) and
   by the route (video sync). Owned at `src/lib/`, not by a feature.
2. `svelte-decorator` --- used by `tag-chip` and `search-interview`. It is one
   mechanism, written once, parameterised by node class and host selector.
3. `shared.ts`'s `formatTimestamp` and `SYNTHETIC_UID_MARKER` --- the first goes
   to `statement/`, the second to `persistence/`. The GraphQL result-shape
   aliases in `shared.ts` are all `ReturnType<typeof useMutation<...>>` gymnastics
   around Apollo hooks; under urql these become plain function types or codegen
   types directly, so most of that file evaporates.

## 2. What evaporates, beyond MIGRATION.md's list

- `ReactExtension` and its `decorators` channel. Per the spike: `mount()` needs
  no root, so the Svelte decorator seam is a plain function returning a
  teardown, dropping straight into `mergeRegister`. Every
  `configExtension(ReactExtension, { decorators: [...] })` dependency entry
  disappears, and with it the awkward situation where `SearchInterviewExtension`
  declares `TagChipPortals` as a dependency of *search*.
- `EditorChildrenComponent: EditorChrome`. Layout composition through extension
  config exists because the React composer owns the tree. In Svelte the route
  writes the chrome as ordinary markup around the contenteditable.
- `useExtensionSignalValue` / `useSignalValue` --- every read site. A rune is a
  signal; there is no bridging hook.
- `TagChipPortals` and `SearchResultPortals` as *components*. They become calls
  to the one decorator seam. Roughly 130 lines of near-duplicated React state
  machinery collapses to two call sites.
- `SearchDriver` as a component. Its only reason for being a component is that
  Apollo's executor lives in a hook. urql's `Client` is a plain object, so the
  search execution moves into `SearchInterviewExtension.register` where the rest
  of the read path already lives. This is the single largest structural
  simplification in the port and should be called out as such.
- `LatencyMeter` via `useExtensionComponent` --- reads a rune, plain component.

## 3. Ownership decisions to settle before code

### 3.1 `playhead`: per-instance, and where it lives

Spike 3 named `playhead` and `searchQuery` together as "the `createModel`
singletons to decouple". That conflates two situations, and they get **opposite**
treatments:

- **`searchQuery` stays a shared singleton.** Settled design decision --- one
  search box, one result set, app-wide. Already ported as `lib/search.svelte.ts`.
  Out of this slice. Do not touch it.
- **`playhead` becomes per-instance.** Its module scope is an artifact of the
  Slate-vs-Lexical bake-off: it was lifted so both editors shared identical
  video-sync machinery and the comparison stayed honest. That bake-off is over
  and the Slate route is gone. One video per interview route, so per-instance is
  the correct model. Module scope only ever worked because two editors are never
  on screen at once.

Four consumers, and they do **not** all reach it the same way. This is the one
design question worth settling before any code:

| consumer | source | needs |
|---|---|---|
| route video sync | `$interviewNumber.tsx` | read `seek`, write `timestamp` |
| `StatementSeekExtension` | `extensions.tsx:198` | write `seek` |
| `UpdateTimestampExtension` | `extensions.tsx:329,339` | read `timestamp` |
| `StatementNode.insertNewAfter` | `nodes.tsx:128` | read `timestamp` |

**Ownership: the route creates it.** It owns the `<video>` element, which is the
only thing that can write `timestamp` authoritatively, and it owns the editor's
lifetime. It creates one playhead per interview and hands it to the editor
extension as config.

**Delivery to the extensions: `defineExtension` config.** The spike measured
config as per-editor (`build()` re-runs per editor, `namedSignals` mints fresh
signals per editor), so this is the channel that is already instance-safe by
construction. `StatementSeekExtension` and `UpdateTimestampExtension` take it
there.

**Delivery to the node: not config.** This is the part that does not fall out of
the pattern, and porter should not try to force it. `insertNewAfter` is a method
on the node class, invoked by Lexical's own `RangeSelection.insertParagraph()`.
Lexical constructs and calls nodes itself, so the node never receives an editor,
a config object, or anything else injectable --- there is no parameter to thread
through and no constructor call site we control.

The seam that does work: `insertNewAfter` is a `$`-function, so it runs with an
active editor. `@lexical/extension` exports `$getExtensionDependency(extension)`,
documented as exactly `getExtensionDependencyFromEditor($getEditor(), extension)`.
The node therefore resolves the *current* editor's statement-extension output and
reads the playhead off it. Per-editor by construction, no ambient import, and it
is the mechanism the extension system ships for precisely this case.

Note the shape this gives the port: the node depends on its own feature's
extension output, not on a module singleton --- which is the same dependency
inversion the extension model applies everywhere else, finally reaching the one
place the React version had to cheat.

### 3.2 How the editor reaches components mounted by the seam

Per the spike: Svelte context does not cross `mount()`. The editor must be
passed explicitly via the decorator's props. No module singleton, no
`editor-context.ts`. This touches every decorator, and it is a rename not a
redesign.

### 3.3 Where urql's client lives

`SearchInterviewExtension` needs to execute a query. Options are (a) import the
module `client` directly, (b) take it through extension config. **Take it
through config**, for the same reason the playhead does: it keeps the extension
testable and keeps the "one module singleton everyone imports" pattern from
regrowing in a new package. The route already has the client from SvelteKit's
load context.

### 3.4 Mutations

Apollo's `useMutation` returns a tuple; the route passes `editStatement` etc.
into `PersistenceExtension` config as opaque executors. Under urql the natural
equivalent is a plain async function closing over the client. Keep the same
shape --- config-injected executor functions --- because it is what makes the
persistence extension independent of the GraphQL library. Do **not** import the
client inside `persistence/`.

## 4. Build order

Numbered to match `MIGRATION.md`'s checklist.

**1. Tooling.** As listed in `MIGRATION.md`. No feature code.

**2. `playhead.svelte.ts`.** Smallest possible commit that proves the runes
module pattern. Per 3.1 this is now a **factory only --- no module instance and
no default export of one.** Exporting a convenience singleton alongside the
factory would leave the old import available, and the first consumer to reach
for it puts the coupling straight back. The absence of a shared instance is the
deliverable.

The consumers do not exist yet at this commit, so this lands as a factory plus
its type. It gets wired in steps 5 and 6.

**3. Framework-neutral core.** `StatementNode` first --- it is the bulk of
`nodes.tsx`, it is a plain `ElementNode` with hand-rolled `createDOM`, and per
the spike it ports by deleting the `.tsx` extension. The **one** change it does
need is `insertNewAfter`'s playhead read, which becomes a
`$getExtensionDependency` lookup per 3.1; everything else is mechanical. Because
that lookup names the statement extension, this commit and the statement
extension in step 5 are mutually referential --- land the node with the lookup
and the extension's output shape together if splitting them would leave a broken
intermediate. Then `TagChipNode` and
`SearchResultNode`, which are structurally identical to each other (both
`MarkNode` + `setDOMUnmanaged` badge + `getDOMSlot().withAfter()`); port them
together and resist the urge to factor a common base class --- they diverge in
styling and in what their decorator renders, and a shared base would couple two
features that should not know about each other.

Land `formatTimestamp` with `StatementNode`.

**4. The Svelte decorator seam.** Port `registerSvelteDecorator` from the spike
essentially verbatim. Two invariants the review will check hard:

- it handles `"updated"` mutations, not only created/destroyed;
- it sweeps on `registerUpdateListener`, re-parenting when `getElementByKey`
  returns a different host.

This is where the one example component test from the constraint budget
should go: the four-gesture table from the spike notes, asserting mount/
unmount counts.

**Correction, made while implementing this step.** The second invariant above
was described as catching root detach/reattach because that gesture
"produces no mutation record at all" --- stated as fact in the spike notes.
Traced against the actual lexical 0.49.0 reconciler source and measured
directly: the detach half (`setRootElement(null)`) is genuinely silent
(`resetEditor` nulls the mutation observer before `$commitPendingUpdates`
runs), but the reattach half fires `FULL_RECONCILE`, which re-announces every
live node as `"created"` regardless -- so the mutation listener alone already
recovers once reattach happens. No test constructed against this Lexical
version shows the sweep changing an outcome for this gesture. The sweep stays
in as insurance against a path this suite does not exercise (a future Lexical
build, or some other gesture not yet found), not as a demonstrated fix for
root detach/reattach specifically. See `svelte-decorator.svelte.ts`'s inline
comment and `test/decorator-seam.test.ts`'s module comment.

**Settle the test file convention as part of this step, before writing the
test.** `vitest.config.ts` currently includes `test/**/*.{test,spec}.{ts,tsx}`.
Two problems, and the second is the dangerous one:

- `.tsx` is dead in this package. Cosmetic.
- The glob is rooted at `test/`, so a test living beside the component it
  covers is not collected. Vitest reports **0 tests, not an error** --- a silent
  pass at precisely the step whose purpose is catching a silent failure. The
  seam test would appear to succeed while never running.

Decision: **keep tests under `test/`, and widen the glob only to drop `tsx`.**
Co-locating is the more common Svelte convention, but it is the wrong trade
here --- the budget is two test files total, `test/` already holds the existing
example, and a glob that reaches into `src/` has to be written carefully around
the naming hazard below for no benefit at this size.

The naming hazard, worth stating because it is not obvious: `*.svelte.ts` is a
**reserved** pattern --- `vite-plugin-svelte` matches `/\.svelte\.[jt]s$/` and
compiles anything matching as a rune module. So a test file must never be named
`something.svelte.ts`. `something.svelte.test.ts` is safe (the suffix breaks the
pattern), but the safest thing at this budget is to sidestep the question
entirely and name the seam test plainly under `test/`.

The component under test is a fixture, not production code, so it can live in
`test/` beside its spec.

**5. Extensions, in dependency order.** `statement` → `statement-seek` →
`update-timestamp` → `tag-chip` → `tag-split-boundary` → `persistence` →
`search-interview` → `latency`. Two or three per commit where they are trivial;
`persistence` and `search-interview` get a commit each.

`persistence` ports with near-zero semantic change --- it is already free of
React, its debounce maps and two-pass `editorState`/`prevEditorState` walk are
framework-neutral, and its only React-shaped dependency is the injected
executors (3.4). The `afterRegistration`-not-`register` placement is
load-bearing and must survive.

`search-interview` is the one that changes shape: `SearchDriver`'s three
concerns (debounced query execution, the query-change effect, the
re-search-on-edit update listener) all move into `register`. The
`useEffect([pendingQuery])` becomes a subscription on the query rune; the
`useRef(debounce(...))` becomes a closure local. Preserve the deliberate
asymmetry the source comments call out: the re-search-on-edit path calls the
debounced handler *directly* rather than writing the query signal, so it does
not reset the focused result.

**6. The route.** `+page.ts` load for the header + transcript queries,
`+page.svelte` for the video, chrome and editor host. `{#if browser}` replaces
`ClientOnly` --- and note the reason is the same one, not a coincidence: no DOM
means no decorator hosts. `{#key interviewUid}` replaces the `useMemo` on the
extension.

**The playhead goes INSIDE the `{#key}` block** --- one playhead per interview,
destroyed and rebuilt when the interview changes. Switching interviews therefore
resets playback position to 0, which is correct: a caption timestamp is only
meaningful against the video it came from, and there is no sense in which
"14:32 of Maxine Wolfe" carries over to Avram Finkelstein.

This is not merely tidier than the alternative --- it fixes a live bug. In the
source, `useVideoSync` is an *unconditional* effect on `seek`: it writes
`player.currentTime = seek.value` whenever `seek` changes. Because the module
singleton survives navigation, `seek` still holds the previous interview's
position when the new route mounts, and the effect fires against the new
`<video>`. Today that is masked by the video's own load sequence rather than by
anything deliberate. Scoping the playhead's lifetime to the interview removes
the stale value instead of racing it.

Concretely: the playhead is created in the same block whose lifetime is
`interviewUid`, alongside the editor extension it is passed to, so the two
cannot disagree about which interview they belong to. If porter finds themselves
hoisting it above the `{#key}` to avoid a re-creation, that is the bug being
reintroduced --- flag it rather than accommodate it.

**7. Remaining routes.** `/` and `/transcript/` are near-duplicates in the
source; collapse to one component per the known-defects list. Theme: the font
`.tsx` files become plain CSS; `styles/theme/*` is a styled-components theme
object with no consumer once styled-components is gone --- port only the tokens
the slice actually reads, and let Spectrum's CSS custom properties carry the
rest. Reconcile `lib/graphql.ts` against the codegen output.

**8. Tests.** One component test = the decorator seam (step 4). One unit test =
`findMatchRanges`, which is pure, has genuinely tricky word-boundary and
regex-escaping behaviour, and whose failure mode is a user-selectable crash.

## 5. Risks, ranked

1. **The decorator sweep.** Silent failure, correctness-critical, and the whole
   port rests on it. Mitigated by the test in step 4.
2. **No template type checking.** `svelte-check` is out. Every `.svelte`
   template expression is unverified until runtime. Practical consequence for
   the build order: keep template logic thin --- derive in the script block,
   render in the template. A `{#each}` over a `$derived` array is reviewable by
   eye; an inline chain of optional accesses is not.
3. **`search-interview`'s reshape.** The largest behavioural surface being moved
   rather than translated. The subtle bits are documented in source comments
   (the `peek` vs `.value` distinction inside the `data` subscriber, the
   `history-merge` + `SEARCH_TAG` pair, the focused-result clamp). Runes have no
   `peek`; reading a `$state` inside a non-tracking context is the equivalent,
   and getting this wrong reintroduces the "typing a new search re-highlights
   the old results" bug. Flag every one of these at review.
4. **The node's playhead lookup.** `$getExtensionDependency` throws if the
   extension is not in the current editor's graph, so `StatementNode` now has a
   hard requirement that the statement extension is registered. That is already
   true everywhere the node is used, but it converts a silent module import into
   a load-bearing graph edge --- worth stating because a future editor built
   with the node and without the extension fails at split time, not at build
   time.

5. **`decorate()` DecoratorNodes.** None exist today. The note is: do not add
   one before writing a Svelte host for it.
6. **Sweep performance at real transcript size.** Unmeasured. If it bites, the
   entry already tracks its last host, so the fix is caching --- not a redesign.

## 6. Explicit non-goals

- Do not port the styled-components theme wholesale.
- Do not port `LatencyExtension`'s console logging or the `console.log` calls
  scattered through `PersistenceExtension` and `SearchDriver`. They are spike
  instrumentation; keep `console.error`/`console.warn` on genuine error paths.
- Do not carry across the essayistic comment register. Where a comment explains
  a real mechanism (the `peek` distinction, the `afterRegistration` ordering,
  the two-state destroy walk), keep a short version.
- Do not build a shared base class for the two `MarkNode` subclasses.
- Do not implement `splitStatement`; the backend gap and the synthetic-uid
  marker port as-is.

## 7. Standing cleanups

Not blocking, and not worth a commit of their own. Fold each into the next
commit that touches the same file.

- **Drop `@types/react` from `package.json`** (currently the only React
  remnant, line 31). Harmless as an unused type package, but while it is
  installed a stray `import type { ... } from "react"` in ported code resolves
  and type-checks cleanly instead of failing. With no `svelte-check` on
  templates, the type errors we *can* still get should be loud. Next
  `package.json` edit.
- **`no-internal-modules` is live, not inert --- corrected, step 5 session.**
  The claim below (this bullet, before correction) was wrong: it is not
  silent, and the fix it proposed was diagnosed against the wrong cause.

  Reproduced firing for real: `yarn oxlint src/lib/editor/`, run from the
  package directory and separately from the repo root, both report the same
  three errors (`StatementNode.ts` -> `persistence/synthetic-uid`,
  `PersistenceExtension.ts` -> `statement/StatementNode`,
  `TagSplitBoundaryExtension.ts` -> `statement/StatementExtension`). The
  earlier "inert" conclusion came from running oxlint at a cwd where
  `import-x/resolver`'s `typescript.project` path
  (`packages/editor-svelte/tsconfig.json`, monorepo-root-relative) does not
  resolve --- the resolver fails silently and the rule fails open there. It is
  not that oxlint supplies no resolver at all; the resolver works, just not
  from every cwd. A rule that fails open depending on cwd is worse than one
  known to be off: it gives different answers to the same question, and the
  wrong answer is the quiet one.

  All three current errors are accepted, intentional cross-feature seams
  (sec 1.2 above), not a backlog. The rule's `allow` list is built around a
  `dir/index.ts` barrel convention that `src/lib/editor/`'s feature
  directories deliberately don't use (71c3e95 declined barrels explicitly) ---
  so the rule is correctly flagging a seam that exists, in a package that
  doesn't mark seams the way the rule expects. No lint script, CI workflow, or
  hook runs oxlint in this package, so none of this has been silently red.

  `forbid` (pure pattern matching, no resolver needed) is still the right
  eventual fix, but designing its patterns now --- with search-interview
  (step 5 commit D), the extension with the most cross-feature surface, still
  unported --- means redesigning them again once that module graph exists.
  Deferred until the graph settles after D lands, sequenced, not abandoned.
  Same defect exists in `packages/editor`.
- **`Temporal` has no polyfill and no lib support** (`editor/statement/
  timestamps.ts`, `formatTimestamp`). Will fail at runtime the first time a
  statement actually renders --- steps 5/6, not step 4. Check whether the
  target runtime ships `Temporal` natively or a polyfill needs adding
  (`temporal-polyfill` / `@js-temporal/polyfill`); same gap exists in
  `packages/editor`, so this isn't new to the port, just newly exercised.

  Note for whoever does this: `--print-config` omits jsPlugin rules wholesale
  and `--deny` is silent even for bogus rule names, so neither can tell a
  passing rule from a disabled one. Probe with a known-bad input instead.

- **Resolved: `assets/` directories now commit normally.** `.git/info/exclude`
  carried a bare `assets` (line 9) *and* `**/assets/**` (line 10) ---
  unanchored, so they matched at any depth, blocking `assets/` directories
  anywhere in the repo from being committed. Same class of bug as the `lib/`
  pattern, but local-only. Fixed and verified.

**Instrument note for all of the above.** `git status` does not show ignored
files, so a "clean tree" from it means nothing about what is hidden. Use
`git status --short --ignored <path>` to see exclusions, and
`git check-ignore -v <file>` to get the rule and line number responsible.
