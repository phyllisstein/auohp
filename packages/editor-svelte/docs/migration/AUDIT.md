# Svelte port audit --- `packages/editor-svelte`

Once-over quality and design audit of the completed React 19 -> Svelte 5 port,
branch `svelte-editor`. Static read-through of every file under `src/` and
`test/`, plus verification against the actual `lexical` / `@lexical/extension`
0.49.0 source in `packages/editor-svelte/node_modules/`. No builds run, no
edits made.

16 findings: 5 P1, 3 P2, 8 P3.

Tags: **design coherence** / **code quality** / **correctness risk**.

---

## P1 --- real, actionable

### 1. PersistenceExtension depends on an invariant it does not own [correctness risk]

`src/lib/editor/persistence/PersistenceExtension.ts:205`

```ts
if (!mutatedNodes?.size) {
    return;
}
```

The entire write path is gated on `mutatedNodes`. But in
`node_modules/lexical/dist/Lexical.dev.mjs:16197-16199`, `setMutatedNode`
returns early when `activeEditor._listeners.mutation.size === 0`:

```js
function setMutatedNode(mutatedNodes, registeredNodes, mutationListeners, node, mutation) {
  if (mutationListeners.size === 0) {
    return;
  }
```

That set is **editor-global**, not per-class (`Lexical.dev.mjs:3732`:
`activeMutationListeners = activeEditor$1._listeners.mutation`). Once it is
non-empty, `setMutatedNode` records *every* mutated node of *every* class;
`triggerMutationListeners` (`:12296-12312`) then does the per-class filtering
when dispatching. So `mutatedNodes` arrives populated for `StatementNode` only
because something else in the graph registered a mutation listener --- namely
the two `registerSvelteDecorator` calls in `tag-chip/TagChipExtension.ts:40`
and `search-interview/SearchInterviewExtension.ts:415`.

**Consequence:** build an editor with `PersistenceExtension` but without
`TagChipExtension` and `SearchInterviewExtension`, and persistence silently
stops saving. No throw, no warning --- the update listener fires on every
commit with `mutatedNodes` undefined and returns at line 205.

This is exactly the "looks right, isn't exercised" class MIGRATION.md's
corrections chase. It is **not** a live bug in `editor.ts` (all three
extensions are wired), and the source had the same latent shape, so it is not
a port regression. But it is an undocumented load-bearing coupling between
three extensions the project's "quarantine each behind a thin adapter"
philosophy says should not know about each other --- and it is the most
consequential thing the port did not write down.

Minimum fix: a comment at `:205` naming the dependency. Robust fix: a no-op
`editor.registerMutationListener(StatementNode, () => {})` inside persistence
so it owns its own precondition. Note the tree already contains the right
idiom for this, applied to a *less* consequential precondition ---
`statement/StatementExtension.ts:21-23` throws at build time on a missing
playhead. The inconsistency is the finding.

### 2. `editor.ts` states the `$initialEditorState` lifecycle backwards [correctness risk / code quality]

`src/lib/editor/editor.ts:70-73`

> Runs once, inside an editor.update() tagged history-merge, after every
> extension's register and before any afterRegistration

It does not. Three corrections, all verified against source:

1. `InitialStateExtension` seeds from its **own `afterRegistration`**, not
   between the two phases (`LexicalExtension.dev.mjs:938-958`).
2. It is root index 0 (`LexicalExtension.dev.mjs:1051`,
   `roots = [normalizeExtensionArgument(InitialStateExtension)]`), so it runs
   **first in the `afterRegistration` loop** (`:1259-1265`) --- before
   Persistence's, not after it.
3. `editor.update()` defers its commit to a microtask
   (`Lexical.dev.mjs:12635`, `scheduleMicroTask(() => $commitPendingUpdates(editor))`).
   `registerEditor`'s two loops (`:1249-1266`) are both fully synchronous, so
   the seed's dirty wave lands after **both** have finished.

This inverts the reasoning in `persistence/PersistenceExtension.ts:90-95`,
which justifies `afterRegistration`-not-`register` on the premise that
"`afterRegistration` runs after that microtask has flushed." It does not.

What *actually* protects persistence from the seed is the
`tags.has("history-merge")` guard at `PersistenceExtension.ts:201` ---
`HISTORY_MERGE_OPTIONS` (`LexicalExtension.dev.mjs:906`) tags the seed update.
The `afterRegistration` placement is still correct (it is the right phase for
other reasons), but the stated reason is wrong, and a future reader who trusts
the comment will conclude the tag guard is redundant belt-and-braces and
delete it.

Same false premise appears at `search-interview/search-output.svelte.ts:136`
("$initialEditorState runs after every register").

### 3. `EditorHost.svelte`'s editor-build `$effect` tracks `contentEditable` [correctness risk]

`src/routes/transcript/[interviewNumber]/EditorHost.svelte:32-38`

```svelte
$effect(() => {
    const editor = buildEditorFromExtensions(extension);
    if (contentEditable) {
        editor.setRootElement(contentEditable);
    }
    return () => editor.dispose();
});
```

`contentEditable` is `$state` written by `bind:this` (`:30`, `:70`). The effect
reads it, so it is a tracked dependency.

On first run `contentEditable` is `undefined`: an editor is built, gets **no
root element**, and its `$initialEditorState` seeds every statement into a
rootless editor. `bind:this` then writes the element, the effect re-runs,
**disposes that first editor and builds a second one.** Every mount constructs
and throws away a complete editor, including one full seed pass over every
statement in the transcript.

The comment above it (`:2-8`) describes the effect as a teardown keyed on
`extension`, which is what it should be. It tracks strictly more than the
comment claims --- precisely the "`$effect` that tracks more than its comment
says" class the brief asked about.

`editor.dispose()` already calls `setRootElement(null)`
(`LexicalExtension.dev.mjs:1119`), so the doubled build is waste rather than a
leak. But on a real transcript it is two full seeds and two decorator
registrations per mount.

Fix: do not read `contentEditable` reactively --- `$effect.pre` plus `untrack`,
or move the build off the bound state entirely.

### 4. `TagSplitBoundaryExtension`'s dangling FIXME is also a live UX dead-end [code quality / correctness risk]

`src/lib/editor/tag-chip/TagSplitBoundaryExtension.ts:43-46`

```ts
// FIXME: Show UX feedback or make a call on splitting the text
// before/after the chip. For now, silently bail in all cases.
event?.preventDefault();
return true;
```

Pressing Enter with the caret inside a tag chip does nothing at all --- no
split, no feedback, no sound. The extension's own header comment (`:17`)
already flags this as "a real open question, not a nicety."

It is the only FIXME in the tree and the only place a user gesture silently
no-ops. Faithful to the source, so not a port defect --- but it is not in
MIGRATION.md's carve-out list the way `/transcript/create` is, and should not
pass a "done" verdict unlogged.

### 5. `{@const}` in the `{#key}` block re-runs on reads beyond `interviewUid` [correctness risk]

`src/routes/transcript/[interviewNumber]/+page.svelte:117-126`

```svelte
{@const playhead = createPlayhead()}
{@const extension = defineAuohpEditorExtension({
    statements, playhead, editStatement, createStatement,
    destroyStatement, searchStatements, interviewUid,
})}
```

`{@const}` is a derived, not a one-shot. `defineAuohpEditorExtension` reads
`statements`, which is `$derived` off `data` (`:38`).

If `data` ever changes without `interviewUid` changing --- an `invalidate()`,
or a client-side re-`load` of the same route, which SvelteKit performs on
`depends()` / `invalidateAll()` --- then the extension object is recreated with
an identical `interviewUid`, `{#key}` does **not** re-run, but `EditorHost`'s
`$effect` tracks `extension` and will dispose and rebuild the editor,
discarding all unsaved in-editor state. `createPlayhead()` at `:117` has the
same exposure: a fresh playhead, video position reset to 0.

Latent today --- nothing in the tree calls `invalidate`. But it sits exactly
where the port's most carefully-reasoned lifetime decision lives, and that
reasoning (`:99-116`, and PLAN.md sec 4 step 6) assumes `{#key interviewUid}`
is the only recreation trigger. It is not.

---

## P2 --- design coherence

### 6. The `$state` / `$effect.root` idiom is applied three different ways [design coherence]

- `search-interview/search-output.svelte.ts:141` --- `$effect.root` with an
  explicit disposer, plumbed back to `register()` through `_dispose` on the
  output object.
- `latency/LatencyExtension.svelte.ts:25` --- `$state` in `build()`, no
  `$effect` at all, so no root and no disposer needed.
- `playhead.svelte.ts:15` --- `$state` in a plain factory, called from a
  component's `{@const}`, so ownership is the component's lifetime.

These are three genuinely different situations and each choice is individually
defensible. The problem is that nothing in the tree says so in one place.
`search-output.svelte.ts:39-49` and `LatencyExtension.svelte.ts:9-13` each
argue their own case locally, from opposite directions, and a reader adding a
seventh extension has no rule to follow --- only two precedents that disagree.

The `_dispose` / `_runQuery` underscore-prefixed escape hatch
(`search-output.svelte.ts:100-103`, `:288`) is a genuine
`build()`-cannot-return-a-teardown workaround and is well explained, but it is
the only one of its kind in the tree and reads as ad hoc rather than as a
pattern.

This is the clearest evidence of different porting sessions drifting into
different idioms for adjacent problems.

### 7. `SearchInterviewExtension` -> `PersistenceExtension` is the weakest seam [design coherence]

`src/lib/editor/search-interview/SearchInterviewExtension.ts:322`

```ts
const { interviewUid } = $getExtensionDependency(PersistenceExtension).output;
```

Search structurally depends on *the write path* to learn which interview it is
searching. The failure mode is silent: `interviewUid` defaults to `""`
(`PersistenceExtension.ts:73`), and search returns unscoped results rather
than throwing.

The extension's own comment (`:285-295`) already flags this, names the silent
failure mode explicitly, and defers the decision to step 6 --- "Worth deciding
at step 6 whether that should change." Step 6 closed without taking the
decision. This finding is mostly "the deferred question is still deferred and
is no longer tracked anywhere."

`editor.ts:39` already threads `interviewUid` in as a top-level option.
Passing it to both extensions independently would remove the cross-feature
edge entirely at the cost of one duplicated wiring line. The comment's stated
worry --- "two extensions each taking their own copy that could disagree" ---
is weaker than the coupling it buys, given `editor.ts` is the single wiring
point for both and they would be adjacent lines in the same object literal.

### 8. `PersistenceOutput` duplicates `PersistenceConfig` field for field [code quality]

`src/lib/editor/persistence/PersistenceExtension.ts:38-64`, `:81-88`

Two interfaces with identical members, and a `build` that copies six fields
one at a time. The comment at `:50-56` carefully explains why the fields are
*not* wrapped in reactive seams, which is genuinely useful --- but it does not
explain why the type is duplicated rather than
`type PersistenceOutput = PersistenceConfig`. Nothing reads the two
differently.

---

## P3 --- nice to have

### 9. Spike scaffolding shipped into the ported search route [code quality]

`src/routes/search/+layout.svelte`

- `:58-60` --- dead `greet()` function whose body is `alert("Welcome to Svelte!")`.
- `:81-106` --- a hardcoded `sp-card` demo: "Card Heading", "JPG Photo", a
  `https://picsum.photos/200/250` placeholder image, and a Photoshop-flavoured
  action menu ("Deselect", "Select Inverse", "Feather...", "Select and
  Mask...", "Make Work Path"). `greet` is wired to the first menu item.
- `:14-16` --- commented-out SWC element registrations.

Also `src/routes/search/+page.svelte:6` --- another commented-out SWC import.

### 10. Stray demo markup in the root layout [code quality]

`src/routes/+layout.svelte:13-19`

An `svelte-ignore` comment describing an `sp-theme` element and its `system`
attribute warning --- but there is no `sp-theme` element in the file. It is
followed by `<button>Hello, button</button>`, which renders on every page of
the application.

### 11. `test/chai.test.ts` is a third test against a two-test budget [code quality]

Asserts `"some string" === "some string"` four ways. Two of the four use
`SOME_STRING.should.equal(...)`, which requires a chai prototype patch
(`chai.should()`) that nothing in the file or the vitest setup installs.

PLAN.md sec 4 step 8 budgets exactly two test files for the whole port (the
decorator seam and `findMatchRanges`, both of which exist and are good). This
is a third, and it is the one carrying no information about the port.

### 12. `@types/react` and `svelte-check` still in `package.json` [code quality]

`package.json:31` (`@types/react`), and `svelte-check` in the same block.

PLAN.md sec 7 lists dropping `@types/react` as a standing cleanup to fold into
the "next `package.json` edit" --- which has since happened (the `test` script
added in `c211828`). While it is installed, a stray
`import type { ... } from "react"` in ported code resolves and type-checks
cleanly instead of failing, which is exactly the loud failure the no-
`svelte-check` situation makes valuable.

`svelte-check` is likewise still a devDependency despite MIGRATION.md
declaring it explicitly out of scope.

### 13. Dead non-null assertions that assert something false [code quality]

- `statement/StatementSeekExtension.ts:52-56` --- `.find(...)!` immediately
  followed by `$isStatementNode(statement) ? ... : null`. `find` returns
  `undefined` when no statement carries that uid, so the `!` is both dead (the
  guard already handles it) and wrong (it asserts non-null of something that
  can be undefined).
- `statement/UpdateTimestampExtension.ts:30` --- `$findMatchingParent(...)!`
  followed by `if (!$isStatementNode(statement)) return false`. Same shape.

Harmless at runtime because the guards are correct. Worth removing because the
assertion states something untrue right next to the code that handles it being
untrue.

### 14. Unused `selection` parameter not marked [code quality]

`search-interview/SearchResultNode.ts:65-69` and
`tag-chip/TagChipNode.ts:75-79` --- both `insertNewAfter(selection, restoreSelection)`
implementations ignore `selection` entirely. TypeScript's
`noUnusedParameters` does not fire on overrides, so it passes silently.

The tree applies the underscore convention consistently elsewhere (`_config`,
`_editor`, `_uid`, `_editorState`); these two are the exception.

### 15. `svelte-decorator.svelte.ts` is named `.svelte.ts` but contains no runes [code quality]

`src/lib/editor/svelte-decorator.svelte.ts` --- `mount` and `unmount` are plain
imports from `"svelte"`. There is no `$state`, `$effect`, `$derived` or
`$effect.root` anywhere in the file.

The extension forces `vite-plugin-svelte` to compile it as a rune module for no
benefit. PLAN.md sec 4 spends a full paragraph on `.svelte.ts` being a
**reserved** filename pattern and a naming hazard to sidestep; this file walks
into it voluntarily. Renaming would touch three import sites
(`TagChipExtension.ts:11`, `SearchInterviewExtension.ts:25`,
`test/decorator-seam.test.ts:11`).

### 16. Comment-to-code ratio in the search-interview feature [code quality]

`search-interview/search-output.svelte.ts` runs roughly 160 lines of comment to
90 of code; `SearchInterviewExtension.ts` is similar.

MIGRATION.md's constraint is "inline comments: terse and informative, no
editorialising," and PLAN.md sec 6 says "do not carry across the essayistic
comment register." Several blocks here --- `:141-155` and `:199-232` are the
clearest --- are the essayistic register in a new accent: they narrate the
porting *decision*, the alternative that was considered, and why it was
rejected. That is MIGRATION.md content, not module content.

The substance is genuinely valuable and hard-won (the `untrack()` reasoning in
particular is correct and non-obvious). Flagging the location, not the content,
and it is a judgment call rather than a defect.

---

## What step 9 will not catch

Findings 1, 2, 3, 5, 6 and 7 are all Svelte- or Lexical-lifecycle specific.

A `modern-web-practices` pass examines the platform surface. It will plausibly
find the `width: 100vw` on `.page`
(`transcript/[interviewNumber]/+page.svelte:182`, which causes horizontal
overflow wherever a classic scrollbar is present), the missing accessibility
story on the `<track>` element, the `picsum.photos` placeholder, and possibly
the `alert()`.

It will not read `Lexical.dev.mjs` to discover that `mutatedNodes` is gated on
an unrelated extension's mutation listener, and it has no way to see that an
`$effect` tracks a `bind:this` target it should not.

---

## Verdict

**Not yet ready to call "done pending step 9's fixes."** Findings 1, 2 and 3
should land first.

- **Finding 3** is a straightforward bug with a small fix and an observable
  cost on every single editor mount.
- **Finding 2** is a comment-only change, but the comment actively misinforms
  about the exact ordering rule the port treats as load-bearing in two separate
  modules.
- **Finding 1** is the one I would insist on. It is a silent-data-loss failure
  mode, it is undocumented, and the tree already contains the correct idiom for
  it (`StatementExtension`'s build-time throw) applied to a *less* consequential
  precondition.

Findings 4-8 are a judgment call. I would log 4 and 7 as explicit carve-outs in
MIGRATION.md --- the way `/transcript/create` was handled in step 7 --- rather
than let a green checkbox absorb them.

Everything in P3 is genuinely nice-to-have. Findings 9, 10 and 11 could
reasonably fold into step 9, since they are all deletions.

**On the port as a whole: it is in good shape.** The feature-scoped extension
decomposition holds together and is a real improvement on the source's
junk-drawer-by-technical-kind layout. The decorator seam is the strongest part
of the tree --- the "move the DOM, not the component" insight is correct, well
tested, and honestly documented about what its test does and does not prove.

Most notably, findings 1 and 2 are both cases where the *same* verification
method this team already established --- read the library source, do not trust
the plausible-sounding claim --- simply had not yet been pointed at these two
specific assertions. The discipline is right; it just has two more places to go.
