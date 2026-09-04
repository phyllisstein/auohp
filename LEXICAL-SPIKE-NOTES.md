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
