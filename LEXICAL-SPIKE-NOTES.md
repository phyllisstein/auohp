# Spike 3 --- Lexical decorator seam in Svelte

Run: `cd spike-svelte-lexical && npm install && npm run dev` --- <http://localhost:5199/>.
Select text, hit **Tag selection**, then hammer the other buttons. Each chip shows
a seconds-alive counter held in the Svelte component's own `$state`. **If the
counter resets, the seam re-mounted and lost state.** The stat row counts
mounts / re-parents / unmounts.

## What was ported

`TagChipExtension` + `TagChipNode` + `TagChipPortals`, end to end --- the
portal-inversion case, i.e. the hard one. Chosen over `LatencyExtension` because
`LatencyExtension` is a `useExtensionComponent` decorator that lives *outside*
the contenteditable and therefore never meets the reconciler; it would have
answered nothing. `MarkNode` internals turned out not to be a problem: the
`$wrapSelectionInMarkNode(selection, false, id, ids => $createTagChipNode(ids))`
call site ported character-for-character.

Files: `src/nodes.ts` (node, verbatim minus JSX), `src/commands.ts` (verbatim),
`src/extensions.ts` (the extension), `src/svelte-extension.svelte.ts` (the seam),
`src/TagChip.svelte`, `src/tag-signals.svelte.ts` (the signal bridge).

## Gating answer: yes, with a two-line trick

**A Svelte-mounted decorator survives Lexical's reconciler.** Measured, in the
running spike:

| gesture | mounts | unmounts | chip state |
| --- | --- | --- | --- |
| mutate `__ids` (`updated` mutation) | 0 | 0 | preserved |
| swap paragraph order (DOM re-parent) | 0 | 0 | preserved |
| type in an adjacent text node | 0 | 0 | preserved |
| `setRootElement(null)` then re-attach --- **every DOM node in the editor destroyed and rebuilt from EditorState** | 0 | 0 | preserved |
| `node.replace(newNode)` --- genuinely new `NodeKey` | +1 | +1 | reset (correct) |

The mechanism, and the answer to "what's the Svelte equivalent of
`createPortal`":

React's `createPortal(vnode, host, key)` keeps a subtree alive across a host
change because the subtree's identity is `(parent fiber, key)` --- the host is
just a render target the reconciler writes into. Svelte 5's `mount()` has no
such indirection: the instance is bound to the `target` it was handed, there is
no `setTarget`, and `unmount()` destroys state.

So don't move the component --- **move the DOM**. `registerSvelteDecorator`
creates one `<span data-svelte-slot>` per `NodeKey`, mounts into it **while it is
still detached** (Svelte does not require the target to be in the document), and
thereafter only ever `host.append(slot)`s it into whatever host Lexical currently
offers. `appendChild` *moves* a live node: every effect, binding, listener and
focus inside it survives, because nothing was ever destroyed. That is exactly the
invariant the portal buys, obtained with one line of DOM instead of a reconciler
feature.

Two things that are not obvious and cost real debugging time:

1. **You must handle `"updated"` mutations, not just `"created"`/`"destroyed"`.**
   The React version skips `updated` (it only tracks the host map); the Svelte
   version must not, because an update is precisely when Lexical may have handed
   the node a new element.
2. **A mutation listener alone is not sufficient.** Lexical rebuilds host DOM in
   cases that produce *no* mutation record for the node in question --- most
   visibly the root detach/reattach above, which re-runs `createDOM` for the
   whole tree while every node stays "unchanged" from the mutation listener's
   point of view. The seam therefore also sweeps on `registerUpdateListener`:
   one `getElementByKey` per live decorator, and re-parent if the host moved.
   This is what makes the fourth row of the table pass. Without it the slots are
   silently orphaned and the chips just vanish.

`setDOMUnmanaged` on the badge is doing all the load-bearing work here and is
completely framework-neutral --- it is the reason a *foreign* framework can
render inside the contenteditable at all. Nothing about the existing node code
had to change.

### The one real regression vs. React

Svelte's `setContext`/`getContext` is component-tree-scoped, and `mount()` starts
a *new root*. React context flows through a portal for free (the portal renders
into the calling component's tree); Svelte context does not cross `mount()`.
So `useLexicalComposerContext()` has no free analogue --- the editor must be
passed explicitly, as a prop on the decorator's props object or via `mount`'s
`context` option. This is a rename, not a redesign: the spike uses a module
singleton (`src/editor-context.ts`) and the real port would pass it in
`DecoratorSpec.props`. Cost: near zero, but it will touch every decorator.

### Bonus: `SvelteExtension` doesn't need to be an extension

`ReactExtension` exists because portals need a React root with a parent fiber, so
something has to own that root and expose a `decorators` channel. `mount()` needs
no root. The Svelte analogue is therefore a plain **function** returning a
teardown --- the exact shape `defineExtension`'s `register` already wants, so it
drops straight into a `mergeRegister(...)`. The dependency-graph entry
`configExtension(ReactExtension, { decorators: [...] })` disappears entirely.
This is the one place the Svelte port is strictly *simpler* than the original.

## `namedSignals` → `$state`

Pleasant, and slightly better than React.

React needs `useExtensionSignalValue(Ext, "stats")` --- a hook call per signal per
component --- plus `@preact/signals-react-transform` wired through
`@rolldown/plugin-babel` just to make bare `.value` reads reactive (see the
existing `project_signals_react_transform` memory; that whole apparatus is
build-config that exists only to paper over React's re-render model).

Svelte's equivalent is a module-level `$state` object in a **`.svelte.ts`** file.
The extension is the entire ceremony: it licenses runes outside a component, and
an exported object becomes a reactive singleton. A component writes
`tagPalette.color` and any template or `$derived` touching it re-runs. No hook,
no extension import in the component, no Babel transform. Verified in the spike:
**Cycle palette** repaints a chip that is mounted outside the Svelte component
tree entirely.

For signals that must stay genuine `namedSignals` `Signal`s (because
framework-agnostic `register()` code writes them), `bridgeSignal()` in
`src/tag-signals.svelte.ts` is a ~10-line signal→signal adapter, written once per
project rather than once per read site. The asymmetry worth naming: React needs
the hook because a component only re-renders when told to; a Svelte rune *is* a
signal, so the adapter is plumbing, not translation.

## `decorate()` vs. portal-inversion: which is the hazard

Counter-intuitively, **portal-inversion is the easier half to port, and
`decorate()` is the hazard.**

- **Portal-inversion (`TagChipNode`, `SearchResultNode`)** --- the node hands you
  a stable, unmanaged host and a mutation listener; you supply the mounting. That
  is entirely framework-choice, which is why one 130-line
  `registerSvelteDecorator` covers *all* of it. Both such nodes in `nodes.tsx` use
  the identical badge/`setDOMUnmanaged`/`getDOMSlot().withAfter()` shape, so they
  port through the same seam with different `resolveHost` selectors. Roughly 40%
  of `nodes.tsx` (~250 of 620 lines), and the seam is now written.
- **`decorate()` (true `DecoratorNode`s)** --- Lexical owns the host *and* the
  lifecycle, but `decorate()` returns **JSX** and `@lexical/react`'s
  `LexicalDecoratorExtension` renders it into a React root that Lexical itself
  manages. There is no Svelte equivalent shipped. Porting means either returning
  a plain DOM element from `decorate()` and mounting into it (workable, and the
  same append trick applies) or forking the decorator host. It is less code but
  it is *unshipped* code, versus portal-inversion which is code we now have.

`StatementNode` --- the bulk of `nodes.tsx` and the piece that actually matters
for AUOHP --- is an `ElementNode` with hand-rolled `createDOM` chrome and no
React in it at all. It ports by deleting the `.tsx` extension.

## Second editor instance

Probed with working code (`src/TwoEditors.svelte`, rendered below the main
editor): two live editors whose graphs both list the same extension objects.
Findings are measured, not inferred.

**1. Extension sharing --- one instance PER EDITOR, not per module.**
`LexicalBuilder` is constructed fresh inside each `buildEditorFromExtensions`
call and owns a private `extensionNameMap: Map<string, ExtensionRep>`. Dedupe is
therefore *within* one editor's graph, keyed by the extension's `name` string.
Measured: `build()` ran **2x** for two editors sharing one `ProbeExtension`.
`defineExtension` returns a config *description*, not an instance --- the
instance is the `ExtensionRep`, and there is one per editor.

**2. `namedSignals` --- per-editor, and that's the good news.** Since `build()`
re-runs, `namedSignals({...})` mints a fresh signal object per editor. Measured:
writing `41` through editor A and `7` through editor B left them reading 41 and 7
respectively --- **independent**. So `SearchInterviewExtension` and
`LatencyExtension` state does *not* collide across two editors. This is the
single most encouraging result: the extension-local state channel is already
instance-safe by construction, for free.

**3. The ambient singletons are the actual problem, and they're outside the
extension system.** `playhead` (`src/playhead.ts`) and `SearchQuery`
(`routes/search/-search-signal.ts`) both use `createModel`, which mints
*per-instance* signals --- but both modules then export a single module-scoped
`new Playhead()` / `new SearchQuery()`. Every importer shares one object. So
`playhead.seek.value = startTime` in `StatementSeekExtension` (~L198), and the
`playhead.timestamp.peek()` reads in `UpdateTimestampExtension` and
`StatementNode`, are genuinely global: **two editors would fight over one
playhead**, and a click-to-seek in the results view would move the transcript
view's video. Note this is a pre-existing coupling that has nothing to do with
Svelte --- React has it identically today.

The fix is small and mechanical, and the code is already shaped for it:
`createModel` exists precisely so you *can* have more than one. Either pass a
`Playhead` instance through extension config (`defineExtension` config is the
natural channel --- it is per-editor by the same mechanism as #2), or keep one
playhead deliberately (two views of *one* video probably *should* share a
playhead) and only split `SearchQuery`. Worth deciding per-singleton rather than
reflexively.

**4. NodeKeys --- no collision, but don't rely on why.** Measured across two
freshly built editors: key spaces were `9..20` and `21..32` --- disjoint, because
Lexical's key counter is **module-global**, not per-editor. The only shared key
is the literal `"root"`. So a single flat `Map<NodeKey, Entry>` happens to be
safe today. It is safe by *accident of a global counter*, though, not by
contract. The seam sidesteps this anyway: `registerSvelteDecorator` is called
from inside `register(editor)`, so each editor gets its **own closure and its own
`entries` map**, and every lookup goes through that editor's
`editor.getElementByKey`. Cross-editor collision is structurally impossible
regardless of the counter. Verified incidentally --- the main seam test still
reports `1 mount / 1 re-parent / 0 unmounts` with three editors live on the page.

**Does this change the port verdict? No --- and Svelte is marginally better
here.** React's `ReactExtension` mounts decorators into a React root whose
context carries the editor, so a second instance means a second provider and the
usual "which context am I in" hazard. Svelte's `mount(Component, { target,
props })` has no ambient context at all --- the editor is passed explicitly. The
context gap called out earlier as the port's one regression turns out to be an
*advantage* under two instances: there is no ambient thing to get wrong. The one
change the real port needs is dropping the `editor-context.ts` module singleton
(fine for a one-editor spike) in favour of putting the editor on
`DecoratorSpec.props`, which the interface already supports.

**Verdict on "make the results page a second editable Lexical instance":
yellow-green.** The extension architecture is already instance-safe where it
counts --- per-editor `build()`, per-editor signals, per-editor decorator maps.
The blocker is not Lexical or Svelte; it is the two hand-rolled module-scoped
`createModel` singletons, which is an afternoon of threading an instance through
config, and a design question (should two views share one playhead?) rather than
a technical risk. Do that decoupling *before* the Svelte port, not during ---
it's orthogonal to the framework and would otherwise muddy the port's diff.

## Verdict: **green**, with one caveat

The seam is real, it is small (~130 lines, once), and it survives the harshest
reconciler gesture available (full root DOM rebuild) with component state
intact. The framework-agnostic 65% is genuinely agnostic --- `nodes.ts`,
`commands.ts` and the command handlers ported without a single semantic change.
`namedSignals` → `$state` is a net simplification that deletes a Babel plugin.

Caveat, and the thing that would most reduce risk: **the update-listener sweep is
correctness-critical and easy to regress.** It is what makes the root-rebuild case
pass, and its failure mode is silent (chips just stop appearing). Two cheap
mitigations, in order:

1. A browser test (vitest-browser is already a devDependency in `packages/editor`)
   asserting mount/unmount counts across the four gestures in the table above.
   Ten minutes of work; it pins the one invariant the whole port rests on.
2. Confirm the sweep is not a perf problem at real transcript size. It is
   `getElementByKey` per live decorator per update; with hundreds of on-screen
   chips during fast typing that wants measuring, and if it bites, the fix is to
   only sweep keys whose element identity actually changed (cache the last
   element per key, which the entry already tracks as `host`).

Remaining unspiked risk, worth naming but not blocking: `decorate()`-based
`DecoratorNode`s have no Svelte host yet. AUOHP has none today that matter, so
this is a "don't add one before writing the host" note, not a port blocker.
