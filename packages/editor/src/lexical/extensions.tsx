import {
    $createTextNode,
    $getNodeByKey,
    $getRoot,
    $getSelection,
    $isRangeSelection,
    COMMAND_PRIORITY_LOW,
    KEY_ENTER_COMMAND,
    configExtension,
    defineExtension,
    safeCast,
    $isTextNode,
    type NodeKey,
    type TextNode,
} from "lexical";
import { namedSignals, type Signal } from "@lexical/extension";
import { HistoryExtension } from "@lexical/history";
import { RichTextExtension } from "@lexical/rich-text";
import { ReactExtension, type EditorChildrenComponentProps } from "@lexical/react/ReactExtension";
import { $dfs, $findMatchingParent, mergeRegister } from "@lexical/utils";
import { debounce } from "perfect-debounce";
import { playhead } from "@/playhead";
import {
    $createStatementNode,
    $adoptStatementIdentity,
    $createTagChipNode,
    $isStatementNode,
    $isTagChipNode,
    $isSearchResultNode,
    $createSearchResultNode,
    TAG_CHIP_BADGE_CLASS,
    STATEMENT_CHROME_CLASS,
    STATEMENT_NODE_CLASS,
    StatementNode,
    TagChip,
    TagChipNode,
    TagMarkStyles,
    SearchResult,
    SearchResultNode,
    SEARCH_RESULT_BADGE_CLASS,
} from "@/lexical/nodes";
import { INSERT_TAG_CHIP_COMMAND, INSERT_SEARCH_RESULT_COMMAND, SEEK_VIDEO_COMMAND } from "@/lexical/commands";
import {
    SYNTHETIC_UID_MARKER,
    type EditStatementFn,
    type TranscriptStatements,
    type SearchStatementsData,
    type DestroyStatementFn,
    type CreateStatementFn,
    type CreateStatementInput,
} from "@/lexical/shared";
import { $unwrapMarkNode, $wrapSelectionInMarkNode, MarkExtension } from "@lexical/mark";
import { useExtensionSignalValue, useSignalValue } from "@lexical/react/useExtensionSignalValue";
import { SEARCH_STATEMENTS_QUERY } from "@/queries";
import { useLazyQuery } from "@apollo/client/react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { useExtensionComponent, useExtensionDependency } from "@lexical/react/useExtensionComponent";
import { createPortal } from "react-dom";
import { useEffect, useState, type JSX, useEffectEvent, useCallback, useMemo, useRef } from "react";
import { Button } from "@react-spectrum/s2/Button";
import { TextField } from "@react-spectrum/s2/TextField";
import SearchIcon from "@react-spectrum/s2/icons/Search";
import { ActionButton, Text } from "@react-spectrum/s2/ActionButton";
import { ActionButtonGroup } from "@react-spectrum/s2/ActionButtonGroup";
import styled from "styled-components";
import { ProgressCircle } from "@react-spectrum/s2/ProgressCircle";
import ChevronUpIcon from "@react-spectrum/s2/icons/ChevronUp";
import ChevronDownIcon from "@react-spectrum/s2/icons/ChevronDown";
import { style } from "@react-spectrum/s2/style" with { type: "macro" };

// -----------------------------------------------------------------------------
// Extensions --- Lexical's composition model as of 0.48.
//
// The old model (now deprecated) was: mount <LexicalComposer initialConfig={...}>
// and hang null-returning React components off it, each calling
// useLexicalComposerContext() to fish the editor back out of context and
// registering listeners in a useEffect. Behaviour was smuggled through the React
// tree, so the editor's capabilities were only knowable by reading JSX.
//
// The new model inverts that: an extension is a plain value built by
// `defineExtension`. It declares what it contributes (`nodes`), what it needs
// (`dependencies`), what it can be tuned with (`config`), what it hands back
// (`build`), and what it does (`register`). The editor is a parameter of
// `register`, not something retrieved from ambient context --- so the whole
// `useLexicalComposerContext` + `useEffect` + `return null` dance disappears.
// React re-enters only where there is genuinely something to paint.
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// StatementExtension --- pure schema.
//
// It contributes StatementNode to the editor and nothing else. Under the old
// model this lived in a distant `initialConfig.nodes` array, structurally
// divorced from the plugins that used it; here the behavioural extensions below
// simply `dependencies: [StatementExtension]`, which both registers the node and
// documents the coupling. Listing it repeatedly is harmless --- the builder
// merges the dependency graph, so a node is registered once no matter how many
// extensions ask for it.
// -----------------------------------------------------------------------------
export const StatementExtension = /* @__PURE__ */ defineExtension({
    name: "@auohp/statement",
    nodes: () => [StatementNode],
});

// -----------------------------------------------------------------------------
// StatementSeekExtension --- click-to-seek, driven by the chrome.
//
// This used to hang off SELECTION_CHANGE_COMMAND, which made *any* arrival in a
// statement seek the video: typing, arrow keys, clicking to place a caret mid-
// sentence. Editing a caption would yank the playhead out from under the editor.
// Seeking is a deliberate act and deserves a deliberate gesture, so the trigger
// is now a click on the timestamp chrome specifically --- and the command carries
// the target uid rather than being inferred from wherever the caret happens to be.
//
// Two mechanisms are worth noting:
//
// 1. The chrome is `setDOMUnmanaged` + contentEditable=false (see StatementNode.
//    createDOM), so Lexical disclaims it entirely --- no selection lands there and
//    no Lexical event machinery reaches it. A plain DOM listener is not a fallback
//    here, it is the only door.
//
// 2. The listener is DELEGATED from the editor's root element rather than attached
//    per-node in createDOM. `createDOM` receives only an EditorConfig and has no
//    editor to dispatch against, so a per-node listener would have to reach for an
//    ambient singleton --- precisely the smell the extension model removes. One
//    root listener also survives reconciliation, which replaces chrome DOM freely.
//
// The command's payload is the uid rather than a timestamp: the handler resolves
// the node and reads its CURRENT startTime, so a seek is always to where the
// statement is now, not to whatever the chrome happened to render when it was
// built.
// -----------------------------------------------------------------------------
export const StatementSeekExtension = /* @__PURE__ */ defineExtension({
    dependencies: [StatementExtension],
    name: "@auohp/statement-seek",

    register (editor) {
        // Hoisted out of the root listener deliberately. `registerRootListener`
        // fires with (nextRoot, prevRoot) on every root change, and removing a
        // listener requires the SAME function reference --- a handler defined
        // inside the callback would be a fresh closure each time and could never
        // be detached, leaking one listener per root swap.
        const onClick = (event: MouseEvent) => {
            const target = event.target;
            if (!(target instanceof Element)) {
                return;
            }

            // Only the chrome column seeks. A click anywhere in the editable
            // content is caret placement and must stay inert.
            const chrome = target.closest(`.${ STATEMENT_CHROME_CLASS }`);
            if (!chrome) {
                return;
            }

            // `data-uid` is kept in sync by StatementNode.updateDOM, including
            // across the synthetic -> real uid adoption after createStatement.
            const uid = chrome.closest(`.${ STATEMENT_NODE_CLASS }`)?.getAttribute("data-uid");
            if (!uid) {
                return;
            }

            editor.dispatchCommand(SEEK_VIDEO_COMMAND, uid);
        };

        return mergeRegister(
            editor.registerCommand(
                SEEK_VIDEO_COMMAND,
                uid => {
                    // The uid identifies the statement; its timing is read live from
                    // the node. `editor.read()` establishes the active editor state
                    // that `getStartTime()` requires --- the command handler runs
                    // outside any update.
                    //
                    // Statements are direct children of root (see the seeding loop in
                    // InitialStateExtension), so this scan is one level deep. It is
                    // linear in statement count, which is the price of keeping the
                    // command's payload a uid --- the identity a toolbar button or a
                    // search result would dispatch with. The click path below could
                    // resolve its node from the DOM in O(1), but that shortcut is not
                    // available to every caller, and a seek happens at human speed.
                    const startTime = editor.read(() => {
                        const statement = $getRoot()
                            .getChildren()
                            .find(node => $isStatementNode(node) && node.getUid() === uid)!;

                        return $isStatementNode(statement) ? statement.getStartTime() : null;
                    });

                    // Non-media statements (broadsheet text) legitimately have no
                    // timing. Seeking to 0 would be worse than not seeking at all.
                    if (startTime == null) {
                        return false;
                    }

                    // Seek to the START of the caption window: the user is asking to
                    // hear this statement, which means from its beginning.
                    playhead.seek.value = startTime;
                    console.debug(`Seeking to statement ${ uid } (${ startTime })`);

                    // `true` --- this command is fully handled here, and nothing
                    // below should also act on it.
                    return true;
                },
                COMMAND_PRIORITY_LOW,
            ),

            editor.registerRootListener((rootElement, prevRootElement) => {
                prevRootElement?.removeEventListener("click", onClick);
                rootElement?.addEventListener("click", onClick);
            }),
        );
    },
});


// -----------------------------------------------------------------------------
// TagChipExtension --- the React-in-editor seam.
//
// This is where the extension model pays off most visibly: the node and the
// command that inserts it are declared together. Under the old model the node
// went in the composer's `initialConfig.nodes` and the command handler went in a
// <TagChipPlugin/> elsewhere in the JSX; nothing tied them together but
// convention and hope.
// -----------------------------------------------------------------------------
export const TagChipExtension = /* @__PURE__ */ defineExtension({
    name: "@auohp/tag-chip",
    nodes: () => [TagChipNode],
    dependencies: [
        MarkExtension,
        configExtension(ReactExtension, { decorators: [TagChipPortals] }),
    ],
    register: editor =>
        editor.registerCommand(
            INSERT_TAG_CHIP_COMMAND,
            id => {
                const selection = $getSelection();
                if (!$isRangeSelection(selection)) {
                    return false;
                }

                // `$wrapSelectionInMarkNode` does the whole selection -> element
                // wrap, including splitting boundary TextNodes. The 4th argument
                // is the factory hook that lets us substitute our subclass for a
                // plain MarkNode --- it receives the accumulated ids, so
                // overlapping tags merge rather than nest.
                $wrapSelectionInMarkNode(selection, false, id, ids => $createTagChipNode(ids));
                return true;
            },
            COMMAND_PRIORITY_LOW,
        ),
});

// -----------------------------------------------------------------------------
// TagSplitBoundaryExtension --- keep proper nouns whole across caption breaks.
//
// StatementNode.insertNewAfter now handles the split itself, so the old
// SplitStatementExtension is gone. What remains is an editorial rule, not a
// structural one: a caption boundary should never fall inside a proper noun.
// Enter halfway through "Larry Kramer" should still split --- just not there.
//
// This has to run before anything mutates. `$removeTextAndSplitBlock` walks up
// splitting nodes until it reaches a block, and a chip is inline
// (INTERNAL_$isBlock requires !isInline()), so it cleaves the chip in two on its
// way to the statement. By the time insertNewAfter is called it is already too
// late to object.
//
// COMMAND_PRIORITY_LOW (1) runs before RichTextExtension's default handler at
// COMMAND_PRIORITY_EDITOR (0) --- the bus dispatches high-to-low. We relocate
// the caret and return `false`, so the default handler proceeds normally; it
// re-reads `$getSelection()` rather than closing over one, so it sees our edit.
// -----------------------------------------------------------------------------
export const TagSplitBoundaryExtension = /* @__PURE__ */ defineExtension({
    dependencies: [StatementExtension, TagChipExtension],
    name: "@auohp/tag-split-boundary",
    register: editor =>
        editor.registerCommand(
            KEY_ENTER_COMMAND,
            event => {
                const selection = $getSelection();

                // Non-collapsed selections replace their content before
                // splitting --- a different problem, left alone for now.
                if (!$isRangeSelection(selection) || !selection.isCollapsed()) {
                    return false;
                }

                const anchor = selection.anchor.getNode();
                const chip = $isTagChipNode(anchor)
                    ? anchor
                    : $findMatchingParent(anchor, $isTagChipNode)!;

                // Caret is not inside a proper noun --- nothing to consolidate.
                if (!$isTagChipNode(chip)) {
                    return false;
                }

                // FIXME: Show UX feedback or make a call on splitting the text
                // before/after the chip. For now, silently bail in all cases.
                event?.preventDefault();
                return true;
            },
            COMMAND_PRIORITY_LOW,
        ),
});

export const UpdateTimestampExtension = /* @__PURE__ */ defineExtension({
    dependencies: [StatementExtension],
    name: "@auohp/update-timestamp",
    register: editor =>
        editor.registerCommand(
            KEY_ENTER_COMMAND,
            event => {
                console.log("UpdateTimestampExtension: KEY_ENTER_COMMAND fired");
                const selection = $getSelection();

                if (!$isRangeSelection(selection) || !selection.isCollapsed()) {
                    return false;
                }

                const anchor = selection.anchor.getNode();
                const statement = $isStatementNode(anchor)
                    ? anchor!
                    : $findMatchingParent(anchor, $isStatementNode)!;

                if (selection.anchor.offset === 0) {
                    console.log("UpdateTimestampExtension: caret at start of statement, updating startTime");
                    editor.update(() => {
                        const currentTime = playhead.timestamp.peek();
                        statement.setStartTime(currentTime);
                    });
                    event?.preventDefault();
                    return true;
                }

                if (selection.anchor.offset === anchor.getTextContentSize()) {
                    console.log("UpdateTimestampExtension: caret at end of statement, updating endTime");
                    editor.update(() => {
                        const currentTime = playhead.timestamp.peek();
                        statement.setEndTime(currentTime);
                    });
                    event?.preventDefault();
                    return true;
                }

                return false;
            },
            COMMAND_PRIORITY_LOW,
        ),
});

// -----------------------------------------------------------------------------
// PersistenceExtension --- the write path.
//
// Two things  changed in the port beyond the mechanical de-componentisation:
//
// 1. It registers in `afterRegistration`, not `register`. Beware the tempting
//    inference here --- it is wrong, and it cost us a bug. The lifecycle is
//    ordered `init -> build -> register -> InitialStateExtension.afterRegistration
//    -> ... -> afterRegistration`, but that ordering governs initiation, not
//    completion. InitialStateExtension seeds via `editor.update()`, and
//    `$beginUpdate` defers its commit to a microtask
//    (`scheduleMicroTask(() => $commitPendingUpdates(editor))`), while
//    `LexicalBuilder.registerEditor` runs both of its loops synchronously.
//    Update listeners fire from `$commitPendingUpdates`, so the seed's dirty-node
//    wave lands after every `afterRegistration` has already returned --- and an
//    unguarded listener sees all of it: one spurious mutation per statement.
//
//    Tags would work (the seed carries HISTORY_MERGE_TAG), but see `lastPersisted`
//    below for why we ask a question about state instead of one about provenance.
//
// 2. `config` replaces props, and `build` turns that config into signals
//    (`namedSignals` --- the same move RichTextExtension and HistoryExtension
//    make). Reading `.peek()` at fire time rather than closing over a value means
//    the debounce delay, or the mutation function itself, can be retuned at
//    runtime from React via `useExtensionDependency(PersistenceExtension)`
//    without tearing down and rebuilding the editor.
//
// The dirty-set -> statement mapping is unchanged from the plugin: Lexical has no
// operation stream (Slate's model), it hands us dirty node sets per update, so we
// walk each dirty node up to its StatementNode ancestor and dedupe by NodeKey.
// -----------------------------------------------------------------------------
export interface PersistenceConfig {
    /** Apollo's `editStatement` executor. `null` disables persistence entirely. */
    editStatement: EditStatementFn | null;
    /** Per-statement debounce window, in milliseconds. */
    delay: number;
    destroyDelay: number;
    createStatement: CreateStatementFn | null;
    destroyStatement: DestroyStatementFn | null;
    interviewUid: string;
}

export const PersistenceExtension = /* @__PURE__ */ defineExtension({
    config: /* @__PURE__ */ safeCast<PersistenceConfig>({
        delay: 1_000,
        destroyDelay: 2_000,
        editStatement: null,
        createStatement: null,
        destroyStatement: null,
        interviewUid: "",
    }),
    dependencies: [StatementExtension],
    name: "@auohp/persistence",

    // `build` must be declared before `afterRegistration`. TypeScript infers this
    // object literal's members in source order, so the Output type (produced here)
    // is only visible to `state.getOutput()` in members that come after it ---
    // otherwise it resolves to `unknown`. Lexical's own InitialStateExtension
    // carries a comment conceding the same ordering constraint.
    build: (_editor, config) => namedSignals(config),

    afterRegistration (editor, _config, state) {
        const { delay, destroyDelay, editStatement, destroyStatement, createStatement, interviewUid } = state.getOutput();

        const createDebouncedUpdate = (uid: string) =>
            debounce((text: string, startTime: number, endTime: number) => {
                // `.peek()` reads the signal without subscribing --- we want the
                // value as of the moment the debounce fires, not as of registration.
                editStatement.peek()?.({
                    variables: { uid, text, startTime, endTime },
                    onCompleted: data => {
                        console.debug(`Edit completed for statement ${ data.editStatement.statement.uid }:`, data.editStatement);
                    },
                });
            }, delay.peek());

        const createDebouncedDestroy = (uid: string) =>
            debounce(() => {
                // `.peek()` reads the signal without subscribing --- we want the
                // value as of the moment the debounce fires, not as of registration.
                destroyStatement.peek()?.({
                    variables: { uid },
                    onCompleted: data => {
                        console.debug(`Destroy completed for statement ${ data.destroyStatement.statement.uid }:`, data.destroyStatement);
                    },
                });
            }, destroyDelay.peek());

        // Takes a NodeKey, never a StatementNode. A node object is a snapshot of one
        // EditorState; this fires from a timer and resolves after a network round
        // trip, so any captured node is stale twice over. The key is the only
        // identity stable across states --- `$getNodeByKey` re-resolves it against
        // whichever state is active at the moment we actually need the node.
        //
        // Note also that `getTextContent()` and friends are `$`-functions wearing a
        // method's clothes: they call `getLatest()`, which requires an active editor
        // state. Hence the `editor.read()` wrapper --- reading the payload out here,
        // unwrapped, is what threw "Unable to find an active editor state".
        const createDebouncedCreate = (_uid: string) =>
            debounce((key: NodeKey) => {
                const payload = editor.read((): CreateStatementInput | null => {
                    const node = $getNodeByKey(key)!;
                    if (!$isStatementNode(node)) {
                        return null;
                    }
                    const startTime = node.getStartTime();
                    const endTime = node.getEndTime();
                    // The schema types both as non-null Float. A statement without
                    // times is not creatable --- narrow here rather than asserting at
                    // the call site.
                    if (startTime === null || endTime === null) {
                        return null;
                    }
                    return { text: node.getTextContent(), startTime, endTime };
                });

                const uid = interviewUid.peek();

                // Either the statement vanished between the edit and the debounce
                // firing, or we have no interview to attach it to.
                if (!payload || !uid) {
                    return;
                }

                createStatement.peek()?.({
                    variables: { statement: payload, interviewUid: uid },
                    onCompleted: data => {
                        console.debug(`Create completed for statement ${ data.createStatement.statement.uid }:`, data.createStatement);
                        editor.update(() => {
                            const node = $getNodeByKey(key)!;
                            if (!$isStatementNode(node)) {
                                return;
                            }
                            $adoptStatementIdentity(node, data.createStatement.statement);
                        }, { tag: "history-merge" });
                    },
                });
            }, delay.peek());

        // One debouncer PER STATEMENT UID, created lazily on first edit. The Slate
        // port used a single shared debouncer, which meant fast edits across two
        // statements cancelled each other's save --- a latent data-loss bug this
        // shape simply cannot have.
        //
        // One map PER OPERATION, though, not one keyed by uid alone. A single map
        // would put create/update/destroy for the same statement in the same slot,
        // so a statement edited before its create had flushed would find the create
        // debouncer under its uid and invoke it with the update's arguments --- a
        // NodeKey parameter receiving a text string. The union type of a shared map
        // reports this as an arity error, which is the type system describing a real
        // aliasing bug rather than an inconvenience to be cast away.
        const updateDebouncers = new Map<string, ReturnType<typeof createDebouncedUpdate>>();
        const destroyDebouncers = new Map<string, ReturnType<typeof createDebouncedDestroy>>();
        const createDebouncers = new Map<string, ReturnType<typeof createDebouncedCreate>>();

        const persistUpdate = (uid: string, text: string, startTime: number, endTime: number) => {
            let flush = updateDebouncers.get(uid);
            if (!flush) {
                flush = createDebouncedUpdate(uid);
                updateDebouncers.set(uid, flush);
            }
            flush(text, startTime, endTime);
        };

        const persistDestroy = (uid: string) => {
            let flush = destroyDebouncers.get(uid);
            if (!flush) {
                flush = createDebouncedDestroy(uid);
                destroyDebouncers.set(uid, flush);
            }
            flush();
        };

        const persistCreate = (statementNode: StatementNode) => {
            const uid = statementNode.getUid();
            let flush = createDebouncers.get(uid);
            if (!flush) {
                flush = createDebouncedCreate(uid);
                createDebouncers.set(uid, flush);
            }
            // Hand over the key, not the node --- see createDebouncedCreate.
            flush(statementNode.getKey());
        };

        const unregister = mergeRegister(
            editor.registerUpdateListener(
                ({ dirtyLeaves, dirtyElements, editorState, tags, mutatedNodes, prevEditorState }) => {
                    console.log(`PersistenceExtension: %o mutations, ${ dirtyLeaves?.size } dirty leaves, ${ dirtyElements?.size } dirty elements, tags: ${ Array.from(tags).join(", ") }`, mutatedNodes);
                    if (tags.has("history-merge")) {
                        return;
                    }

                    if (!mutatedNodes?.size) {
                        return;
                    }

                    // A destroyed node cannot be resolved against `editorState` ---
                    // being absent from it is what "destroyed" MEANS. But Lexical's
                    // states are persistent data structures: the previous tree is
                    // intact and structurally shared, so the node is still fully
                    // readable in `prevEditorState` under the same key. That is where
                    // its uid --- the only identifier the server knows --- survives.
                    //
                    // Hence two passes over two states rather than one. `read()`
                    // installs its state as the active one for the duration of the
                    // callback, so a key can only be resolved from inside the pass
                    // for the state that contains it; flipping mid-walk would be
                    // both confusing and wrong.
                    const destroyedKeys: NodeKey[] = [];

                    editorState.read(() => {
                        const seen = new Set<NodeKey>();

                        const collect = (key: NodeKey, update: "updated" | "created" | "destroyed") => {
                            if (update === "destroyed") {
                                destroyedKeys.push(key);
                                return;
                            }

                            const node = $getNodeByKey(key);
                            if (!node) {
                                return;
                            }
                            const statement = $isStatementNode(node)
                                ? node
                                : $findMatchingParent(node, $isStatementNode)!;
                            if (!$isStatementNode(statement) || seen.has(statement.getKey())) {
                                return;
                            }
                            seen.add(statement.getKey());

                            const uid = statement.getUid();

                            if (update === "created") {
                                // "Created" is also what an undone deletion looks
                                // like: the statement reappears with the real uid it
                                // already had on the server. Sending `createStatement`
                                // for it would mint a duplicate row while the queued
                                // destroy went ahead and removed the original.
                                //
                                // A non-synthetic uid is precisely the signal that
                                // this row already exists server-side, so the correct
                                // response is to cancel the pending destroy and treat
                                // the resurrection as a no-op.
                                //
                                // `destroyDelay` is therefore the entire undo window,
                                // and deliberately longer than `delay`: an undone
                                // deletion is recoverable only while the destroy is
                                // still queued. Past that, the row is gone and an undo
                                // leaves the statement visible in the editor but absent
                                // from the graph --- it takes neither branch below,
                                // since re-creating it would mint a duplicate under a
                                // fresh uid, orphaning its span and :SAYS edge.
                                //
                                // Closing that gap properly needs a tombstone and a
                                // restore mutation. Declined at statement granularity:
                                // it buys seconds of undo for a permanent obligation on
                                // every read path. The intended answer is a draft /
                                // explicit-save model, where deletions stay local until
                                // committed and this race stops existing --- with
                                // tombstones reserved for whole transcripts, where the
                                // loss actually warrants them.
                                const pendingDestroy = destroyDebouncers.get(uid);
                                if (pendingDestroy && !uid.includes(SYNTHETIC_UID_MARKER)) {
                                    pendingDestroy.cancel();
                                    destroyDebouncers.delete(uid);
                                    console.log(`PersistenceExtension: statement ${ uid } restored before its destroy flushed, cancelling`);
                                    return;
                                }

                                console.log(`PersistenceExtension: statement ${ uid } created, persisting`);
                                persistCreate(statement);
                                return;
                            }

                            if (update === "updated") {
                                console.log(`PersistenceExtension: statement ${ uid } updated, persisting`);
                                const text = statement.getTextContent();
                                const startTime = statement.getStartTime()!;
                                const endTime = statement.getEndTime()!;
                                persistUpdate(uid, text, startTime, endTime);
                            }
                        };

                        for (const [_klass, val] of mutatedNodes.entries()) {
                            for (const [key, status] of val.entries()) {
                                collect(key, status);
                            }
                        }
                    });

                    if (destroyedKeys.length) {
                        prevEditorState.read(() => {
                            for (const key of destroyedKeys) {
                                const node = $getNodeByKey(key)!;

                                // Only statements are persisted, and --- unlike the
                                // pass above --- we deliberately do NOT walk up to a
                                // parent statement. Deleting a word destroys TextNodes
                                // inside a statement that is still very much alive;
                                // that arrives separately as an `updated` mutation on
                                // the statement itself. Treating a destroyed child as
                                // a destroyed statement would delete the row the user
                                // was merely editing.
                                if (!$isStatementNode(node)) {
                                    continue;
                                }

                                const uid = node.getUid();

                                // A statement born of a split carries a synthetic uid
                                // until `createStatement` answers with a real one. Two
                                // cases, and they need opposite handling:
                                if (uid.includes(SYNTHETIC_UID_MARKER)) {
                                    // The uid is still synthetic, so the server has
                                    // never heard of this statement --- `destroyStatement`
                                    // would 404. But a create may be pending in the
                                    // debounce window, and letting it fire would create
                                    // a row for a statement that no longer exists.
                                    // Cancelling is the whole of the work here: the
                                    // create never happens, so no destroy is needed.
                                    const pendingCreate = createDebouncers.get(uid);
                                    pendingCreate?.cancel();
                                    createDebouncers.delete(uid);

                                    console.log(`PersistenceExtension: synthetic statement ${ uid } destroyed before creation, cancelling pending create`);
                                    continue;
                                }

                                // A real uid: either seeded from the server, or adopted
                                // by `$adoptStatementIdentity` when a create completed.
                                // Cancel any pending create anyway --- harmless if
                                // absent, and it closes the window where a create that
                                // has not yet flushed races the destroy.
                                createDebouncers.get(uid)?.cancel();
                                createDebouncers.delete(uid);

                                // Any queued edit is moot once the row is going away.
                                updateDebouncers.get(uid)?.cancel();
                                updateDebouncers.delete(uid);

                                console.log(`PersistenceExtension: statement ${ uid } destroyed, persisting`);
                                persistDestroy(uid);
                            }
                        });
                    }
                },
            ),
        );

        // The old plugin leaked here: its useEffect cleanup dropped the Map
        // without cancelling in-flight timers. An extension's disposer is the
        // natural place to do that properly.
        return () => {
            unregister();
            for (const map of [updateDebouncers, destroyDebouncers, createDebouncers]) {
                for (const flush of map.values()) {
                    flush.cancel();
                }
                map.clear();
            }
        };
    },
});


// -----------------------------------------------------------------------------
// SearchInterviewExtension --- the read path, and a worked example of the one
// mechanism that carries live data into an extension.
//
// The tempting-but-broken shape was to hand Apollo's `useLazyQuery` tuple to
// this extension as config. It cannot work, and the reason is worth internalising
// because it generalises to every "how do I update my extension" question:
//
//   `LexicalBuilder` calls `build(editor, config)` exactly once, at editor
//   construction. `namedSignals(config)` then copies each config value into a
//   fresh signal at that instant. There is no later re-apply pass, because there
//   is no re-render for an extension --- an extension is a value, not a
//   component. So config is the signal's seed; the signal is the channel.
//   `namedSignals`' own docstring concedes this: it exists "so it can be
//   reconfigured at runtime".
//
// Worse, trying to force it through config poisons the editor's lifetime. The
// route must `useMemo` the extension, `LexicalExtensionComposer` memoises the
// editor on that extension's identity and disposes the old one --- so putting a
// per-render Apollo result in the dep array rebuilds the editor mid-search and
// throws away the user's unsaved edits and caret.
//
// Hence: this extension takes no config and owns its query outright. Data flows
// one way, and every hop is a signal write:
//
//   PERFORM_SEARCH_COMMAND -> `query` signal
//                          -> SearchDriver (React) runs Apollo
//                          -> `data` / `loading` signals
//                          -> consumers (React, or register-time subscribers)
//
// The command handler stays a pure state write --- it never touches Apollo. That
// is what keeps it synchronous (Lexical command handlers must return a boolean
// immediately) and what makes "what does this command do" answerable without
// knowing anything about the network.
// -----------------------------------------------------------------------------
export interface SearchOutput {
    /** The pending search string. `null` means idle --- no search requested yet. */
    query: Signal<string | null>;
    /** Latest results, or `undefined` before the first response. */
    data: Signal<SearchStatementsData>;
    /** Whether a search is currently in flight. */
    loading: Signal<boolean>;
    /** The index of the result that has the caret, or `null` if none. */
    focusedResult: Signal<number | null>;
    /** The total number of results, or `0` if none. */
    resultCount: Signal<number>;
    /**
     * The marks painted by the last highlight pass, in document order.
     *
     * This is the authority for what "result N" means, and it exists because
     * neither of the two things that previously stood in for it is correct.
     * `resultCount` was the number of matching STATEMENTS, but a statement
     * saying "ACT UP ... ACT UP" carries two marks, so the counter and the
     * highlights disagreed about the total. And `SearchResultPortals` indexed
     * its `hosts` Map, whose insertion order is the order mutations happened to
     * arrive from the reconciler --- not document order.
     *
     * Written by `$applySearchResults`, which is the one moment when the marks
     * and their order are both known for certain.
     */
    resultKeys: Signal<readonly NodeKey[]>;
    /** The replacement string. Empty is legal --- it means "delete the match". */
    replacement: Signal<string>;
}

// A character range within a statement's flattened text, half-open: `[start, end)`.
export interface MatchRange {
    start: number;
    end: number;
}


// Where the highlight ranges come from.
//
// The server cannot tell us. `db.index.fulltext.queryNodes` scores whole
// Statement nodes against the Lucene index and returns the node --- the
// token -> character-offset mapping Lucene built while analysing the text is
// internal to the index and never surfaces through Cypher. `SearchHit` carries
// `statement { uid, text }`, and that is the whole of it.
//
// So the ranges are recomputed here, from the text we already have. That is
// only defensible because the index is created with no analyzer argument
// (`CREATE FULLTEXT INDEX statementText ... ON EACH [s.text]` in api/src/main.rs),
// which means Neo4j's default `standard` analyzer: it lowercases and splits on
// non-word boundaries, but does NOT stem and does NOT strip stopwords. Had the
// index been built with the `english` analyzer, "organizing" would index as the
// stem "organ" and match a statement reading "organized" --- and a literal scan
// for "organizing" would find nothing to highlight in a statement that
// legitimately matched.
//
// One divergence survives and is accepted by design: the fragment is sent to
// Lucene unquoted, so a multi-word selection parses as OR'd terms and a
// statement matching only one of them is still a hit. Such a statement is
// returned with no literal occurrence of the full fragment, and therefore gets
// no highlight. Closing that gap belongs at the query (phrase-quoting the
// fragment in SearchDriver), not here.
// Escape every character the RegExp grammar treats as special, so a selection
// containing `(`, `.`, `?`, `[` and friends is matched literally rather than
// compiled as a pattern. Without this, selecting "ACT UP (1987)" throws
// SyntaxError on the unbalanced group --- a user-selectable crash.
//
// `$&` in the replacement is the whole match, so this is "prefix every special
// character with a backslash" with no capture group needed.
const escapeRegExp = (literal: string) => literal.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

// `\b` is a zero-width assertion between a `\w` and a non-`\w`, so it only means
// what we want when the fragment's own edge characters are word characters.
// Anchoring "(1987)" with `\b` on the left would demand a word character before
// the `(` and never match. So each boundary is applied conditionally, per end.
const WORD_EDGE = /\w/;

function findMatchRanges (text: string, fragment: string): MatchRange[] {
    // A zero-length fragment makes a global RegExp match the empty string at
    // every position, yielding N zero-width ranges and an infinite loop in any
    // hand-rolled scan. There is also nothing to highlight.
    const needle = fragment.trim();
    if (needle.length === 0) {
        return [];
    }

    const pattern = new RegExp(
        (WORD_EDGE.test(needle.at(0)!) ? "\\b" : "") +
        escapeRegExp(needle) +
        (WORD_EDGE.test(needle.at(-1)!) ? "\\b" : ""),
        // `g` to find every occurrence, `i` because Lucene's `standard` analyzer
        // lowercases both sides --- a statement returned for "act up" may well
        // read "ACT UP", and matching case-sensitively would render a hit with no
        // highlight at all.
        //
        // Doing this with a RegExp rather than `text.toLowerCase().indexOf(...)`
        // is the load-bearing choice: `toLowerCase` is not length-preserving in
        // general (U+0130 LATIN CAPITAL LETTER I WITH DOT ABOVE lowercases to two
        // code units), so offsets found in the lowercased copy can drift out of
        // alignment with `text`. `matchAll` reports `index` in the ORIGINAL
        // string's coordinates, which is exactly what $markMatchesInStatement
        // needs.
        "gi",
    );

    const ranges: MatchRange[] = [];
    let lastEnd = 0;

    for (const match of text.matchAll(pattern)) {
        const start = match.index;
        const end = start + match[0].length;

        // Drop anything that overlaps the previous accepted range. `matchAll`
        // already advances past each match so a fixed-length literal cannot
        // self-overlap, but the invariant is asserted here rather than assumed:
        // $markMatchesInStatement derives splitText cut points from these
        // boundaries, and overlapping ranges would produce cuts that interleave
        // into nonsense pieces.
        if (start < lastEnd) {
            continue;
        }

        ranges.push({ start, end });
        lastEnd = end;
    }

    return ranges;
}


// Stamped on the `editor.update()` that paints search highlights, so listeners
// can tell the search's own writes apart from a human's typing. Without it the
// re-search-on-edit listener in SearchDriver would react to the repaint it just
// caused and spin forever --- the marks it watches are destroyed and recreated
// on every result set.
const SEARCH_TAG = "auohp-search-highlight";


// Contract: given a result set (or `undefined`, meaning "no search"), leave the
// document holding exactly the marks that set implies --- nothing stale from the
// previous search, nothing missing from this one.
//
// The marks are now INLINE. Previously this wrapped every child of a matching
// StatementNode in a single SearchResultNode, so the mark was a container for
// the whole paragraph and the highlight was a full-width band behind it. Now a
// statement gets one mark per literal occurrence of the query, wrapping only the
// matched run:
//
//   before: statement -> SearchResultNode -> [all children]
//   after:  statement -> [Text("We shut "), Mark -> [Text("ACT UP")], Text(" down")]
//
// `fragment` is threaded in as a parameter rather than read from the `query`
// signal inside. The signal is the LATEST request; `results` is the response to
// some earlier one, and under a fast second search those are different strings.
// Highlighting a response with a query it did not answer is the classic
// stale-closure bug, and passing both together makes them impossible to
// desynchronise.
// Returns the keys of every mark it painted, in document order --- the caller
// stores this as `resultKeys`, which is what makes "jump to result N" and the
// "N of M" counter agree with each other and with the page.
//
// The order is free rather than earned: `root.getChildren()` walks statements
// top to bottom, and `$markMatchesInStatement` marks occurrences left to right
// within one, so appending as we go is already document order. Recovering it
// afterwards would mean a second full traversal.
function $applySearchResults (results: SearchStatementsData, fragment: string | null): readonly NodeKey[] {
    const uids = new Set(results?.search.statementText.map(({ statement }) => statement.uid) ?? []);

    const root = $getRoot();
    const keys: NodeKey[] = [];

    for (const child of root.getChildren()) {
        if (!$isStatementNode(child)) {
            continue;
        }
        const statement = child;

        $clearSearchResults(statement);

        if (fragment !== null && uids.has(statement.getUid())) {
            keys.push(...$markMatchesInStatement(statement, fragment));
        }
    }

    return keys;
}


// Remove every SearchResultNode beneath `statement`, hoisting its children back
// into the parent, then heal the text runs the unwrap leaves behind.
//
// The old version only looked at direct grandchildren, which was sufficient when
// a mark WAS the statement's only child. Inline marks sit at arbitrary depth
// among the text, so the search has to be a traversal.
function $clearSearchResults (statement: StatementNode): void {
    // Collect before mutating: `$dfs` walks live node versions, and unwrapping
    // during the walk invalidates the cursor it is holding.
    const marks = $dfs(statement)
        .map(({ node }) => node)
        .filter($isSearchResultNode);

    for (const mark of marks) {
        $unwrapMarkNode(mark);
    }

    if (marks.length > 0) {
        $mergeAdjacentTextNodes(statement);
    }
}


// Unwrapping a mark hoists its TextNode children up beside their former
// siblings, so `[Text("We shut "), Mark[Text("ACT UP")], Text(" down")]` becomes
// three sibling TextNodes where the document logically has one run. Left
// unmerged, every search/clear cycle shatters the paragraph further, and the
// offsets `findMatchRanges` returns (which are relative to the statement's whole
// text) stop lining up with any single node.
//
// `mergeWithSibling` is the counterweight, and it does the same quiet work
// `splitText` does in the other direction: it rebases any RangeSelection
// anchor/focus pointing into the absorbed node onto the survivor. `isSimpleText`
// is the guard --- it is false for TextNodes carrying format/style/mode, and
// merging those would silently drop the formatting of one side.
function $mergeAdjacentTextNodes (statement: StatementNode): void {
    let previous: TextNode | null = null;

    for (const child of statement.getChildren()) {
        if ($isTextNode(child) && child.isSimpleText()) {
            if (previous !== null) {
                previous = previous.mergeWithSibling(child);
                continue;
            }
            previous = child;
        } else {
            previous = null;
        }
    }
}


// Wrap each occurrence of `fragment` in `statement` in its own SearchResultNode.
//
// Offsets from `findMatchRanges` are relative to the statement's FLATTENED text
// (`statement.getTextContent()`), but the text lives in one or more TextNodes and
// may be interrupted by TagChipNodes. So the walk below re-derives each child's
// span in flattened coordinates and intersects it with the ranges --- which is
// also why ranges are processed per-child rather than per-range.
function $markMatchesInStatement (statement: StatementNode, fragment: string): readonly NodeKey[] {
    const ranges = findMatchRanges(statement.getTextContent(), fragment);
    if (ranges.length === 0) {
        return [];
    }

    const uid = statement.getUid();
    const keys: NodeKey[] = [];
    let offset = 0;

    for (const child of statement.getChildren()) {
        const size = child.getTextContentSize();
        const childStart = offset;
        const childEnd = offset + size;
        offset = childEnd;

        if (!$isTextNode(child) || !child.isSimpleText()) {
            continue;
        }

        // Ranges intersecting this child, clamped into child-local coordinates
        // and clipped to its bounds --- a range straddling a TagChip boundary
        // highlights the part that falls in this node and is dropped elsewhere.
        const local = ranges
            .filter(({ start, end }) => start < childEnd && end > childStart)
            .map(({ start, end }) => ({
                start: Math.max(start, childStart) - childStart,
                end: Math.min(end, childEnd) - childStart,
            }))
            .filter(({ start, end }) => end > start);

        if (local.length === 0) {
            continue;
        }

        // `splitText` takes cut points, not ranges: the flattened, deduped,
        // in-bounds boundaries. It returns the resulting nodes left-to-right, and
        // --- the part worth internalising --- it remaps any RangeSelection
        // anchor/focus that pointed into the original node onto the correct piece
        // with a rebased offset. Rebuilding the run by hand with setTextContent
        // would teleport the user's caret on every search.
        const cuts = [...new Set(local.flatMap(({ start, end }) => [start, end]))]
            .filter(cut => cut > 0 && cut < size)
            .sort((a, b) => a - b);

        const pieces = child.splitText(...cuts);

        // Walk the pieces alongside the same cut boundaries to decide which are
        // matches. A piece starting at a range's start is a match; the boundary
        // list and the piece list are in lockstep by construction.
        const starts = new Set(local.map(({ start }) => start));
        let pieceOffset = 0;

        for (const piece of pieces) {
            const pieceStart = pieceOffset;
            pieceOffset += piece.getTextContentSize();

            if (!starts.has(pieceStart)) {
                continue;
            }

            // One mark per occurrence, all carrying the statement's uid, so the
            // badge portal and any future "jump to hit N" affordance can still
            // resolve back to the statement that matched.
            //
            // `insertBefore` + `append`, NOT `piece.replace(mark)`, and the
            // difference is the user's caret. `replace` is selection-aware, but
            // its remap for a point anchored on the replaced node is
            // `$moveSelectionPointToEnd(anchor, mark)` --- and at that instant
            // the mark is still EMPTY, because `piece` has not been appended
            // yet. "End of an empty element" resolves to the element-anchored
            // point (mark, 0); the subsequent append re-homes the text but
            // nothing re-derives the selection, so a caret sitting inside a run
            // that becomes a match jumps to the front of its new mark. (Type
            // "GMaichC" back to "GMHC" over a live search and watch it happen.)
            //
            // Reparenting sidesteps the guess entirely: `piece` is never
            // destroyed, so no point anchored on it ever needs relocating.
            // Same principle as preferring `splitText` to `setTextContent`
            // above --- move nodes, never recreate the ones selection names.
            const mark = $createSearchResultNode([uid]);
            piece.insertBefore(mark);
            mark.append(piece);
            keys.push(mark.getKey());
        }
    }

    return keys;
}


// Replace the text inside one mark, leaving the surrounding run intact.
//
// The mark is an ElementNode wrapping one or more TextNodes, so "replace the
// match" means: put `replacement` into the first child, drop the rest, then
// unwrap. Unwrapping is what makes this a real edit rather than a re-highlight
// --- the mark described a match that no longer exists once the text changes,
// and leaving it would strand a highlight around text that does not match.
//
// `$mergeAdjacentTextNodes` afterwards is the same healing step $clearSearchResults
// performs, and for the same reason: the hoisted children arrive as siblings of
// the runs they were split out of, and an unmerged paragraph shatters further
// with every replace.
//
// Returns the containing statement so callers can report what changed.
function $replaceMark (mark: SearchResultNode, replacement: string): StatementNode | null {
    // `$findMatchingParent` already narrows to StatementNode via its type-guard
    // overload, so this is a null check rather than a second type test --- the
    // guard itself will not accept a nullable argument.
    const statement = $findMatchingParent(mark, $isStatementNode);
    if (statement === null) {
        return null;
    }

    const children = mark.getChildren();
    const [first, ...rest] = children;

    if ($isTextNode(first)) {
        // setTextContent on the surviving child, rather than building a fresh
        // TextNode, so that a selection anchored inside this node is rebased by
        // Lexical instead of being left pointing at a node that no longer exists.
        first.setTextContent(replacement);
        for (const child of rest) {
            child.remove();
        }
    } else {
        // No text child to reuse (a mark containing only a TagChip, say). Insert
        // the replacement as a new node ahead of whatever is there and clear the
        // rest, which keeps the branch total rather than silently doing nothing.
        mark.append($createTextNode(replacement));
        for (const child of children) {
            child.remove();
        }
    }

    $unwrapMarkNode(mark);
    $mergeAdjacentTextNodes(statement);

    return statement;
}


export const SearchInterviewExtension = /* @__PURE__ */ defineExtension({
    nodes: () => [SearchResultNode],
    dependencies: [
        StatementExtension,
        configExtension(ReactExtension, { decorators: [TagChipPortals, SearchResultPortals, SearchDriver] }),
    ],
    name: "@auohp/search-interview",

    // The return type is annotated rather than inferred, and that is load-bearing
    // for a reason that has nothing to do with documentation: `dependencies` above
    // names `SearchDriver`, and `SearchDriver`'s body asks for this extension's
    // output. Left to inference that is a cycle TypeScript refuses to resolve.
    // Annotating here (and annotating SearchDriver's return type) cuts it in both
    // directions --- neither side needs the other's body to compute its type.
    build: (): SearchOutput => namedSignals({
        query: null as string | null,
        data: undefined as SearchStatementsData,
        loading: false,
        focusedResult: null,
        resultCount: 0,
        resultKeys: [] as readonly NodeKey[],
        replacement: "",
    }),

    register (editor, _config, state) {
        const { query, data, focusedResult, resultCount, resultKeys } = state.getOutput();

        // Preact's `subscribe` invokes its callback immediately with the current
        // value. At registration that value is `undefined` and the document is not
        // even seeded yet ($initialEditorState runs after every `register`), so the
        // first call is noise. Swallowing it explicitly beats guarding on
        // `results === undefined` inside the subscriber, because `data` legitimately
        // returns to `undefined` later and that case must still clear the marks.
        let primed = false;

        // `mergeRegister` folds several disposers into one. The previous version
        // returned nothing from `register`, so both command handlers outlived the
        // editor --- revisiting the route stacked a second handler on the same
        // command, and one click fired two searches.
        return mergeRegister(
            // The read path's terminus, and note that no React is involved: signals
            // are subscribable anywhere, and `register` already holds the editor.
            // React appears in this extension only where something is painted
            // (SearchResultPortals) or where a hook is unavoidable (SearchDriver).
            data.subscribe(results => {
                if (!primed) {
                    primed = true;
                    return;
                }

                // `history-merge` is load-bearing, not decoration. Wrapping text in
                // a MarkNode leaves `getTextContent()` byte-identical, so
                // PersistenceExtension would happily fire an `editStatement` per
                // highlighted statement, saving text that never changed. It skips
                // this tag --- and search highlighting is genuinely not a user edit,
                // so it should not enter the undo stack as one either.
                // `query.peek()`, not `query.value`: this callback is already a
                // subscriber to `data`, and reading `.value` here would enrol it
                // as a subscriber to `query` too --- so merely typing a new
                // search would re-run the highlight pass against the OLD results.
                // `peek` reads without subscribing.
                // SEARCH_TAG rides alongside: `history-merge` says "this is not a
                // user edit" (to persistence and undo), while SEARCH_TAG says who
                // wrote it, so SearchDriver's update listener can decline to
                // re-search in response to its own highlight pass. Tags compose ---
                // the two claims are orthogonal and both are needed.
                //
                // The highlight pass is also where the result COUNT is settled,
                // rather than in `onSearchUpdate` where it used to live. The
                // response only knows how many statements matched; a statement
                // reading "ACT UP ... ACT UP" is one hit to the server and two
                // marks on the page, and the navigation UI means the second thing.
                // Counting what was painted is the only way the two agree.
                editor.update(
                    () => {
                        const keys = $applySearchResults(results, query.peek());

                        resultKeys.value = keys;
                        resultCount.value = keys.length;

                        // Clamp rather than reset. A re-search triggered by the
                        // user editing the transcript must not throw them back to
                        // the first hit --- they are typically standing on the hit
                        // they just edited. Only fall back to 0 when there was no
                        // position to keep, and to null when nothing matched.
                        const focused = focusedResult.peek();
                        focusedResult.value = keys.length === 0
                            ? null
                            : Math.min(focused ?? 0, keys.length - 1);
                    },
                    { tag: ["history-merge", SEARCH_TAG] },
                );
            }),

            editor.registerCommand(
                INSERT_SEARCH_RESULT_COMMAND,
                id => {
                    const selection = $getSelection();
                    if (!$isRangeSelection(selection)) {
                        return false;
                    }

                    // `$wrapSelectionInMarkNode` does the whole selection -> element
                    // wrap, including splitting boundary TextNodes. The 4th argument
                    // is the factory hook that lets us substitute our subclass for a
                    // plain MarkNode --- it receives the accumulated ids, so
                    // overlapping marks merge rather than nest.
                    $wrapSelectionInMarkNode(selection, false, id, ids => $createSearchResultNode(ids));
                    return true;
                },
                COMMAND_PRIORITY_LOW,
            ),
        );
    },
});


// -----------------------------------------------------------------------------
// LatencyExtension --- litmus test 1 instrumentation, and the canonical shape for
// "an extension that also has a face".
//
// `build` is the hook for producing values other code consumes: it returns an
// `output` object, here a `stats` signal plus a `Component`. An extension whose
// output carries a `Component` satisfies `OutputComponentExtension`, which is
// exactly what `useExtensionComponent` consumes --- so React reaches the meter by
// asking the extension for it, rather than the meter reaching into React context
// for the editor. The dependency arrow reverses.
//
// Note also that `lastKeystrokeAt` is now a closure local rather than a module
// global. Under the old plugin, two editors on one page shared that variable and
// silently corrupted each other's measurements; per-editor `register` scope fixes
// that for free.
// -----------------------------------------------------------------------------
export interface LatencyStats {
    count: number;
    lastMs: number;
    maxMs: number;
}

export interface LatencyOutput {
    stats: Signal<LatencyStats>;
    Component: () => JSX.Element;
}

export const LatencyExtension = /* @__PURE__ */ defineExtension({
    build: (): LatencyOutput => ({
        ...namedSignals({ stats: { count: 0, lastMs: 0, maxMs: 0 } as LatencyStats }),
        Component: LatencyMeter,
    }),
    dependencies: [ReactExtension],
    name: "@auohp/latency",
    register (editor, _config, state) {
        const { stats } = state.getOutput();

        // Lexical is UNCONTROLLED: the ContentEditable owns its DOM and React does
        // not re-render per keystroke (the exact opposite of Slate's controlled
        // <Editable/>). To make that observable we stamp `performance.now()` on
        // each `beforeinput` and measure the gap to the resulting EditorState
        // update.
        let lastKeystrokeAt = 0;
        const stamp = () => {
            lastKeystrokeAt = performance.now();
        };

        const unregisterRoot = editor.registerRootListener((rootEl, prevRootEl) => {
            prevRootEl?.removeEventListener("beforeinput", stamp);
            rootEl?.addEventListener("beforeinput", stamp);
        });

        const unregisterUpdate = editor.registerUpdateListener(() => {
            if (lastKeystrokeAt === 0) {
                return;
            }
            const delta = performance.now() - lastKeystrokeAt;
            lastKeystrokeAt = 0;
            // Writing `.value` notifies subscribers; the React meter re-renders,
            // and nothing else does --- which is the property under test.
            const { count, maxMs } = stats.peek();
            stats.value = {
                count: count + 1,
                lastMs: delta,
                maxMs: Math.max(maxMs, delta),
            };
        });

        return () => {
            unregisterRoot();
            unregisterUpdate();
        };
    },
});


// -----------------------------------------------------------------------------
// The root extension.
//
// This single value replaces the entire old `initialConfig` object and the pile
// of <Plugin/> children: `namespace`/`onError` were initialConfig fields,
// `dependencies` were JSX children, `nodes` are contributed by the dependencies
// themselves, and `$initialEditorState` replaces SeedPlugin outright.
//
// It is a factory rather than a constant because the seed data is per-interview.
// Because `$initialEditorState` is declared here it closes directly over
// `statements` --- no config plumbing, no `$getExtensionDependency` lookup.
// -----------------------------------------------------------------------------
// Note what is not here any more: nothing search-related. Every option in this
// interface must be stable for the editor's entire lifetime, because the route
// has to memoise the returned extension and any change to it destroys the
// document. Live data belongs in signals, not here.
export interface AuohpEditorOptions {
    statements: TranscriptStatements;
    editStatement: EditStatementFn;
    createStatement: CreateStatementFn;
    destroyStatement: DestroyStatementFn;
    interviewUid: string;
}

export function defineAuohpEditorExtension ({ statements, editStatement, createStatement, destroyStatement, interviewUid }: AuohpEditorOptions) {
    return defineExtension({
        dependencies: [
            configExtension(PersistenceExtension, { editStatement, createStatement, destroyStatement, interviewUid }),
            SearchInterviewExtension,
            configExtension(ReactExtension, { EditorChildrenComponent: EditorChrome }),
            HistoryExtension,
            LatencyExtension,
            RichTextExtension,
            StatementExtension,
            StatementSeekExtension,
            TagChipExtension,
            TagSplitBoundaryExtension,
            UpdateTimestampExtension,
        ],
        name: "@auohp/editor",
        namespace: "auohp-lexical-spike",
        onError: (error: Error) => {
            throw error;
        },

        // Runs once, inside an `editor.update()` tagged for history-merge, after
        // every extension's `register` and before any `afterRegistration`. That
        // ordering is what lets PersistenceExtension drop the old "seed" tag check.
        $initialEditorState () {
            const root = $getRoot();
            root.clear();

            // Extending SerializedElementNode ultimately makes it more
            // difficult than creating them one by one
            //
            // let sn = StatementNode.importJSON(statement);

            // This is also not a thing: as above, type requires more than a
            // big bag of node JSON.
            //
            // RootNode.importJSON(statements);

            const appendables: StatementNode[] = [];
            for (const statement of statements) {
                const statementNode = $createStatementNode(
                    statement.uid,
                    statement.startTime,
                    statement.endTime,
                );
                const textNode = $createTextNode(statement.text);
                statementNode.append(textNode);
                appendables.push(statementNode);
            }
            root.append(...appendables);

            return root;
        },
    });
}

function LatencyMeter (): JSX.Element {
    // `useExtensionSignalValue` bridges the signal to React via
    // useSyncExternalStore --- no editor, no context, no useEffect.
    const stats = useExtensionSignalValue(LatencyExtension, "stats");

    return (
        <div style={{ fontFamily: "monospace", fontSize: "0.8rem", opacity: 0.8 }}>
            edits: { stats.count } | last: { stats.lastMs.toFixed(2) }ms | max: { stats.maxMs.toFixed(2) }ms
        </div>
    );
}

// A trivial toolbar affordance that dispatches the typed insert command ---
// demonstrating an out-of-editor React control mutating EditorState, which then
// renders back through a React DecoratorNode. `useLexicalComposerContext` is not
// deprecated: it remains the sanctioned way for a component that genuinely
// renders something to reach the editor (useExtensionComponent is built on it).
// What was deprecated is using it as a back door for behaviour-only components.
function TagButton (): JSX.Element {
    const [editor] = useLexicalComposerContext();

    // The payload is the mark ID. Deferred decision: a throwaway uid for now, so
    // each chip is at least distinct. When entity resolution lands, this becomes
    // the graph uid of the Person/Organization being mentioned --- MarkNode's
    // __ids then genuinely means "this range mentions these entities", and
    // $getMarkIDs answers that question directly. No signature change needed.
    return (
        <Button
            id="insert-tag-chip"
            type="button"
            onPress={ () => editor.dispatchCommand(INSERT_TAG_CHIP_COMMAND, crypto.randomUUID()) }>
            Insert #person chip
        </Button>
    );
}

const SearchContainer = styled.div`
    position: fixed;
    z-index: 1000;
    right: 0;

    display: flex;
    flex-direction: column;
    gap: 1rem;
    align-items: flex-end;
    justify-content: center;

    width: 100%;
    height: max-content;
    min-height: max-content;
    padding: 1rem;
`;

const SearchFieldContainer = styled.div`
    width: 50%;
`;

const ButtonGroupContainer = styled.div`
    display: flex;
    align-items: center;
    justify-content: flex-end;
    width: max-content;
`;

function SearchBar (): JSX.Element {
    const { query, focusedResult, resultKeys, replacement } = useExtensionDependency(SearchInterviewExtension).output;
    const queryValue = useExtensionSignalValue(SearchInterviewExtension, "query");
    const loading = useExtensionSignalValue(SearchInterviewExtension, "loading");
    const resultCount = useExtensionSignalValue(SearchInterviewExtension, "resultCount");
    const focusedResultValue = useExtensionSignalValue(SearchInterviewExtension, "focusedResult");
    const replacementValue = useExtensionSignalValue(SearchInterviewExtension, "replacement");
    const [editor] = useLexicalComposerContext();

    const spinner = (
        <ProgressCircle
            aria-label="Loading…"
            value={ 80 }
            isIndeterminate
            size="S"
            staticColor="white" />
    );

    // Both directions wrap. `%` after adding `resultCount` keeps the operand
    // non-negative --- JavaScript's `%` is a remainder, not a modulus, so a bare
    // `(current - 1) % n` yields -1 at the top of the list rather than n-1.
    //
    // The guard matters because `resultCount` now counts painted marks, which is
    // 0 whenever the query matches nothing; without it both handlers would
    // compute NaN and poison the signal. The buttons are disabled in that state,
    // but a keyboard shortcut bound to these later would not be.
    const focusNext = useCallback(() => {
        if (resultCount === 0) {
            return;
        }
        focusedResult.value = ((focusedResult.peek() ?? -1) + 1) % resultCount;
    }, [focusedResult, resultCount]);

    const focusPrevious = useCallback(() => {
        if (resultCount === 0) {
            return;
        }
        focusedResult.value = ((focusedResult.peek() ?? 0) - 1 + resultCount) % resultCount;
    }, [focusedResult, resultCount]);

    // Replace the focused match.
    //
    // Note what is NOT here: no tag. Every other `editor.update` in this
    // extension carries `history-merge` to tell PersistenceExtension "no text
    // changed, do not save" --- true of the highlight pass, which only wraps
    // runs in marks. Replace is the opposite and must stay untagged so that all
    // three downstream listeners fire: persistence saves the statement, history
    // records an undo step, and the re-search listener repaints the results.
    //
    // That last one is why nothing here re-runs the search by hand. Unwrapping
    // the mark dirties a leaf, the update listener sees an untagged commit with
    // dirty leaves, and the debounced search follows on its own.
    const replaceFocused = useCallback(() => {
        const index = focusedResult.peek();
        if (index === null) {
            return;
        }

        editor.update(() => {
            const key = resultKeys.peek()[index];
            if (key === undefined) {
                return;
            }

            const mark = $getNodeByKey(key) ?? undefined;
            if (!$isSearchResultNode(mark)) {
                return;
            }

            $replaceMark(mark, replacement.peek());
        });

        // Hold the index rather than advancing it. The replaced match leaves the
        // result set, so the NEXT match slides into this position --- keeping the
        // index puts the user on it, which is what repeated Replace clicks want.
        // Clamping against the new count happens in the highlight pass.
    }, [editor, focusedResult, resultKeys, replacement]);

    // Replace every match, in one update so it is one undo step and one save per
    // statement rather than one per occurrence.
    //
    // Iterating the key list is safe even though each `$replaceMark` unwraps a
    // mark: `resultKeys` is a plain array captured before the walk, and the keys
    // it names are independent nodes. Resolving each key inside the loop (rather
    // than resolving all the nodes up front) means a mark already removed as a
    // side effect of an earlier replacement simply misses.
    const replaceAll = useCallback(() => {
        const keys = resultKeys.peek();
        if (keys.length === 0) {
            return;
        }

        editor.update(() => {
            const value = replacement.peek();

            for (const key of keys) {
                const mark = $getNodeByKey(key) ?? undefined;
                if (!$isSearchResultNode(mark)) {
                    continue;
                }
                $replaceMark(mark, value);
            }
        });
    }, [editor, resultKeys, replacement]);
    return (
        <SearchContainer>
            <SearchFieldContainer>
                <TextField
                    aria-label="Search transcript"
                    type="search"
                    enterKeyHint="search"
                    inputMode="search"
                    prefix={ loading ? spinner : <SearchIcon /> }
                    size="M"
                    value={ queryValue ?? "" }
                    onChange={ value =>
                        // Writing the signal is the request; SearchDriver is what makes it a network call.
                        query.value = value.length > 0 ? value : null } />
            </SearchFieldContainer>
            <ButtonGroupContainer>
                { resultCount > 0 && (
                    <span style={{ padding: "0 1rem" }} className={ style({ color: "detail", fontSize: "detail" }) }>
                        { (focusedResultValue ?? 0) + 1 } of { resultCount }
                    </span>
                ) }
                <ActionButtonGroup isDisabled={ queryValue === null || loading || !resultCount }>
                    <ActionButton onPress={ focusPrevious }>
                        <ChevronUpIcon />
                        <Text>Previous</Text>
                    </ActionButton>
                    <ActionButton onPress={ focusNext }>
                        <ChevronDownIcon />
                        <Text>Next</Text>
                    </ActionButton>
                </ActionButtonGroup>
            </ButtonGroupContainer>
            <SearchFieldContainer>
                <TextField
                    aria-label="Replace with"
                    type="text"
                    inputMode="text"
                    size="M"
                    value={ replacementValue }
                    onChange={ value => replacement.value = value } />
            </SearchFieldContainer>
            <ButtonGroupContainer>
                { /* Deliberately not disabled on an empty replacement: clearing
                     a match is a legitimate edit, and "replace with nothing" is
                     how you delete a repeated filler word across a transcript. */ }
                <ActionButtonGroup isDisabled={ queryValue === null || loading || !resultCount }>
                    <ActionButton onPress={ replaceFocused }>
                        <Text>Replace</Text>
                    </ActionButton>
                    <ActionButton onPress={ replaceAll }>
                        <Text>Replace All</Text>
                    </ActionButton>
                </ActionButtonGroup>
            </ButtonGroupContainer>
        </SearchContainer>
    );
}

// ReactExtension renders `<>{contentEditable}{children}</>` by default, which
// would put our toolbar below the transcript. Overriding EditorChildrenComponent
// through `configExtension` is how the new model does layout composition --- the
// editor's chrome is configuration of an extension rather than JSX the route
// happens to nest in the right order.
function EditorChrome ({ contentEditable, children }: EditorChildrenComponentProps): JSX.Element {
    const Meter = useExtensionComponent(LatencyExtension);

    return (
        <>
            <TagMarkStyles />
            <SearchBar />
            <div style={{ display: "flex", gap: "1rem", alignItems: "center", padding: "0.5rem 0" }}>
                <TagButton />
                <Meter />
            </div>
            { contentEditable }
            { children }
        </>
    );
}

// -----------------------------------------------------------------------------
// TagChipPortals --- the ElementNode -> React bridge.
//
// A DecoratorNode gets React for free: the reconciler pulls JSX out of
// `decorate()` at the node's own position. TagChipNode extends MarkNode, hence
// ElementNode, so no such hook exists --- and it must stay an ElementNode,
// because its children are the tagged text (a DecoratorNode's getTextContent()
// returns "", which would silently erase those words from the statement text
// PersistenceExtension ships to the server).
//
// So we invert the flow and push instead: a mutation listener reports every
// chip's lifecycle, we resolve each one's unmanaged badge span, and React
// portals into it. The badge being `setDOMUnmanaged` is what makes this legal
// --- Lexical's mutation-attribution up-walk terminates there, so nothing React
// renders inside gets evicted as foreign DOM.
//
// Rendered via ReactExtension's `decorators` channel, which exists precisely for
// "JSX inside the editor context that is not location-dependent".
// -----------------------------------------------------------------------------
function TagChipPortals (): JSX.Element {
    const [editor] = useLexicalComposerContext();

    // NodeKey -> the badge span to portal into. Held in React state (not a ref)
    // because adding or dropping an entry must trigger a re-render.
    const [hosts, setHosts] = useState<ReadonlyMap<NodeKey, HTMLElement>>(new Map());

    useEffect(
        () =>
            editor.registerMutationListener(TagChipNode, mutations => {
                // Resolve a chip's portal target from its NodeKey. Returns null
                // when the node has no DOM yet (or no badge, e.g. a node
                // replacement swapped createDOM out from under us).
                const resolveHost = (key: NodeKey): HTMLElement | null =>
                    editor.getElementByKey(key)?.querySelector<HTMLElement>(
                        `:scope > .${ TAG_CHIP_BADGE_CLASS }`,
                    ) ?? null;

                setHosts(prev => {
                    let updates = 0;
                    const mutablePrev = new Map(prev);

                    for (const mutation of mutations) {
                        const [key, kind] = mutation;

                        if (kind === "updated") {
                            continue;
                        }

                        if (kind === "destroyed" && mutablePrev.has(key)) {
                            mutablePrev.delete(key);
                            updates++;
                            continue;
                        }

                        const host = resolveHost(key);

                        if (kind === "created" && !!host) {
                            mutablePrev.set(key, host);
                            updates++;
                        }
                    }

                    if (updates === 0) {
                        return prev;
                    }

                    return mutablePrev;
                });
            }),
        [editor],
    );

    return (
        <>
            { Array.from(hosts, ([key, host]) => createPortal(<TagChip nodeKey={ key } />, host, key)) }
        </>
    );
}

// Deliberately does not read the `data` signal. It reacts to SearchResultNode
// mutations, which is a strictly later event: `$applySearchResults` creates the
// nodes, the reconciler builds their badge spans, the mutation listener fires,
// and only then is there anything to portal into. Reading `data` here as well
// would close a loop --- create nodes -> mutation -> setHosts -> re-render ->
// create nodes --- and conflate "own the marks" with "paint inside the marks".
function SearchResultPortals (): JSX.Element {
    const [editor] = useLexicalComposerContext();

    // NodeKey -> the badge span to portal into. Held in React state (not a ref)
    // because adding or dropping an entry must trigger a re-render.
    const [hosts, setHosts] = useState<ReadonlyMap<NodeKey, HTMLElement>>(new Map());
    const focusedResult = useExtensionSignalValue(SearchInterviewExtension, "focusedResult");
    const resultKeys = useExtensionSignalValue(SearchInterviewExtension, "resultKeys");

    // Which NodeKey is focused, resolved through the ordered key list rather than
    // by indexing `hosts`. `hosts` is a Map filled in mutation-arrival order ---
    // reconciler order, not document order --- so its Nth entry is not reliably
    // the Nth match down the page. Comparing keys sidesteps the question of what
    // order this Map happens to be in.
    const focusedKey = focusedResult === null ? null : resultKeys[focusedResult] ?? null;

    useEffect(
        () =>
            editor.registerMutationListener(SearchResultNode, mutations => {
                // Resolve a chip's portal target from its NodeKey. Returns null
                // when the node has no DOM yet (or no badge, e.g. a node
                // replacement swapped createDOM out from under us).
                const resolveHost = (key: NodeKey): HTMLElement | null =>
                    editor.getElementByKey(key)?.querySelector<HTMLElement>(
                        `:scope > .${ SEARCH_RESULT_BADGE_CLASS }`,
                    ) ?? null;

                setHosts(prev => {
                    let updates = 0;
                    const mutablePrev = new Map(prev);

                    for (const mutation of mutations) {
                        const [key, kind] = mutation;

                        if (kind === "updated") {
                            continue;
                        }

                        if (kind === "destroyed" && mutablePrev.has(key)) {
                            mutablePrev.delete(key);
                            updates++;
                            continue;
                        }

                        const host = resolveHost(key);

                        if (kind === "created" && !!host) {
                            mutablePrev.set(key, host);
                            updates++;
                        }
                    }

                    if (updates === 0) {
                        return prev;
                    }

                    return mutablePrev;
                });
            }),
        [editor],
    );

    return (
        <>
            { Array.from(hosts, ([key, host]) => createPortal(<SearchResult focused={ key === focusedKey } nodeKey={ key } />, host, key)) }
        </>
    );
}

// -----------------------------------------------------------------------------
// SearchDriver --- the React half of the search seam.
//
// Apollo's executor only exists inside a hook, so something has to be a component.
// But note what this component is not: it renders nothing, it takes no props, and
// the route neither knows it exists nor passes anything to it. It is registered
// through ReactExtension's `decorators` channel --- the same channel TagChipPortals
// and SearchResultPortals use --- which means "mount this inside the editor's React
// context". Search became a capability the editor has, rather than something the
// route configures it with, and the dependency arrow reversed accordingly.
//
// Reaching its own extension's output via `useExtensionDependency` is legal here
// for the same reason TagChipPortals may call `useLexicalComposerContext`:
// decorators render inside the composer, long after the extension graph is built.
//
// `useSignalValue` (not `useSignalEffect` from @preact/signals-react) is the right
// subscriber: it is `useSyncExternalStore`-based, so it needs no signals-react
// babel/swc transform and participates correctly in concurrent rendering. Both
// libraries do resolve to the single `@preact/signals-core` copy in node_modules,
// so a Lexical extension signal and a `playhead` signal are the same kind of thing.
// -----------------------------------------------------------------------------
function SearchDriver (): JSX.Element | null {
    const { query, data, loading, focusedResult, resultCount, resultKeys } = useExtensionDependency(SearchInterviewExtension).output;
    const [editor] = useLexicalComposerContext();

    // Subscribing to `query` is what turns a command dispatch into a re-render of
    // this component --- and nothing else in the editor re-renders, which is the
    // whole point of routing live data through signals instead of through props.
    const pendingQuery = useSignalValue(query);

    const [runSearch, searchState] = useLazyQuery(SEARCH_STATEMENTS_QUERY, {
        fetchPolicy: "network-only",
    });

    const onSearchUpdate = useEffectEvent(() => {
        loading.value = false;

        console.log("SearchDriver: onSearchUpdate fired with state", searchState);
        const { data: searchData } = searchState;

        if (searchState.error) {
            console.error("SearchDriver: search error", searchState.error);
        }
        if (searchData && !searchData.search.statementText) {
            console.warn("SearchDriver: onSearchUpdate fired with data but no search.statementText field", searchData);
        }
        if (searchData && searchData.search.statementText) {
            console.log("SearchDriver: onSearchUpdate fired with results", searchData.search.statementText);

            // Only `data` is set here. `resultCount` and `focusedResult` used to
            // be derived from `statementText.length` at this point, which counted
            // matching STATEMENTS --- but every consumer of those signals means
            // occurrences. They are now settled by the highlight pass in
            // `register`, which is downstream of this write and can count the
            // marks it actually painted.
            data.value = searchData;
            console.log("SearchDriver: data.value updated to", data.peek());
        }
    });

    const debouncedQueryHandler = useRef(debounce(async (query: string) => {
        if (query !== "") {
            try {
                console.log("SearchDriver: debouncedQueryHandler running with query", query);
                const res = await runSearch({
                    variables: {
                        fragment: `"${ query }"`,
                    },
                });
                console.log("SearchDriver: runSearch returned", res);
                onSearchUpdate();
            } catch (error) {
                console.warn("SearchDriver: runSearch error", error);
                loading.value = false;
            }
        }
    }, 1_500, { leading: false, trailing: true }));

    // Fires only when the user types in the search box --- a NEW query, for which
    // discarding the old position is right. The re-search-on-edit listener below
    // deliberately calls `debouncedQueryHandler` directly instead of writing
    // `query.value`, precisely so it does not land here and reset a position the
    // user is standing on. Routing that path through this signal would look like
    // a simplification and would silently reintroduce the jump-to-first-hit bug.
    useEffect(() => {
        focusedResult.value = null;
        resultCount.value = 0;
        resultKeys.value = [];

        if (!pendingQuery) {
            data.value = undefined;
            loading.value = false;
            return;
        }
        loading.value = true;
        debouncedQueryHandler.current(pendingQuery);
    }, [pendingQuery]);

    // Re-run the search when the human edits the transcript, so the result set
    // and its highlights stay honest about the text actually on screen.
    //
    // This CANNOT be a mutation listener on SearchResultNode, for two reasons
    // worth stating because both are easy to walk back into:
    //
    //   1. Mutations do not bubble. Typing inside a highlighted run mutates the
    //      mark's TextNode child, not the mark; typing anywhere else mutates no
    //      mark at all. The one class guaranteed NOT to see ordinary edits is
    //      the one wrapping the matches.
    //   2. $applySearchResults destroys and recreates every mark on each result
    //      set. A listener that re-searches on mark mutations therefore feeds
    //      itself --- search, repaint, mutation, search --- forever.
    //
    // registerUpdateListener sees every commit and, crucially, its `tags`, which
    // is how an editor distinguishes a human's write from its own. SEARCH_TAG is
    // stamped on the highlight pass so this listener can decline to react to it.
    useEffect(
        () =>
            editor.registerUpdateListener(({ tags, dirtyLeaves }) => {
                // Cheapest predicate first, and it is also the most decisive: with
                // no query there is no result set to keep honest, so an edit made
                // with the search bar closed costs nothing at all. Note this must
                // not fall through to `debouncedQueryHandler` with an empty string
                // --- the handler no-ops on "", but only AFTER the debounce has
                // already scheduled a timer, which would displace a pending real
                // search. `peek`, not `.value`: a listener is not a reactive
                // context, and subscribing here would be meaningless anyway.
                const pending = query.peek();
                if (!pending) {
                    return;
                }

                // Our own highlight pass, and the initial document seed, are not
                // the human changing the transcript. Reacting to SEARCH_TAG in
                // particular is the infinite loop described above.
                if (tags.has(SEARCH_TAG) || tags.has("history-merge")) {
                    return;
                }

                // Selection-only commits --- caret moves, clicks, focus changes ---
                // arrive here constantly and dirty nothing. `dirtyLeaves` is the
                // honest signal for "text actually changed"; `dirtyElements` is
                // not, because it always contains `root` (every commit reconciles
                // from the top), so testing it would make this gate vacuous.
                if (dirtyLeaves.size === 0) {
                    return;
                }

                // Deliberately unscoped: any text edit anywhere re-runs the search.
                // The narrower "did this edit touch a mark" test cannot see the two
                // cases that matter most --- growing a match from adjacent text
                // ("I [ACT UP] in" -> "I [ACT UP]ped in", where the dirty leaf is
                // the neighbour, not the mark), and typing a brand-new match into a
                // statement that has never held one. The trailing debounce already
                // collapses a burst of keystrokes into a single round-trip, so the
                // cost of being permissive is one query per typing pause.
                debouncedQueryHandler.current(pending);
            }),
        [editor, query],
    );

    // Move the caret to the focused result.
    //
    // Subscribing to the signal directly, rather than reading it through
    // `useExtensionSignalValue`, is deliberate: this component renders nothing,
    // and hooking the value into render would make every Next/Previous click
    // re-render it for no visual purpose. Moving the caret is a side effect on
    // the editor, so it belongs on the subscription, not on a render pass.
    //
    // Scrolling is NOT done here --- `SearchResult` already scrolls itself into
    // view when its `focused` prop flips (see nodes.tsx). That split is worth
    // keeping: scroll position follows declaratively from which mark is focused,
    // while the selection is an imperative act on the document.
    useEffect(
        () =>
            focusedResult.subscribe(index => {
                if (index === null) {
                    return;
                }

                editor.update(
                    () => {
                        // Re-read inside the update rather than closing over the
                        // array. A debounced re-search may have repainted every
                        // mark between the click and this callback, which makes
                        // the old keys stale --- and a stale key is not an error
                        // here, just a miss, so `$getNodeByKey` returning null is
                        // an ordinary outcome to bail on rather than to guard
                        // against upstream.
                        const key = resultKeys.peek()[index];
                        if (key === undefined) {
                            return;
                        }

                        // `?? undefined` because the type guard is written against
                        // `LexicalNode | undefined` while `$getNodeByKey` returns
                        // `| null` --- the two spellings of "absent" meet here.
                        const mark = $getNodeByKey(key) ?? undefined;
                        if (!$isSearchResultNode(mark)) {
                            return;
                        }

                        // Select the matched run rather than collapsing to a
                        // caret at its edge. This is what Cmd-G does in most
                        // editors, it makes the current hit visible as a
                        // selection even before any highlight styling, and it
                        // leaves the document one keystroke from replacing the
                        // match --- which is the seam find-and-replace will use.
                        mark.select(0, mark.getChildrenSize());
                    },
                    // The caret move is our write, not the human's. Untagged it
                    // would reach the re-search listener above; that listener
                    // happens to ignore it (a selection change dirties no
                    // leaves), but relying on that coincidence is how the loop
                    // comes back the next time the gate is edited.
                    { tag: ["history-merge", SEARCH_TAG] },
                );
            }),
        [editor, focusedResult, resultKeys],
    );

    return null;
}
