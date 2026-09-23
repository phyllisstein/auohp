import { $getNodeByKey, defineExtension, safeCast, type NodeKey } from "lexical";
import { namedSignals, type Signal } from "@lexical/extension";
import { $findMatchingParent, mergeRegister } from "@lexical/utils";
import type { useMutation } from "@apollo/client/react";
import { debounce } from "perfect-debounce";
import type {
    CreateStatementMutation,
    CreateStatementMutationVariables,
    DestroyStatementMutation,
    DestroyStatementMutationVariables,
    EditStatementMutation,
    EditStatementMutationVariables,
} from "~/__generated__/queries.gql";
import { StatementExtension } from "~/lexical/statement/StatementExtension";
import { $adoptStatementIdentity, $isStatementNode, type StatementNode } from "~/lexical/statement/StatementNode";
import { SYNTHETIC_UID_MARKER } from "./synthetic-uid";


// Executor contracts: the extension's boundary with whoever wires it. Derived
// from Apollo's hook types so they track the operations' actual signatures.
export type EditStatementFn = ReturnType<typeof useMutation<EditStatementMutation, EditStatementMutationVariables>>[0];
export type CreateStatementFn = ReturnType<typeof useMutation<CreateStatementMutation, CreateStatementMutationVariables>>[0];
export type DestroyStatementFn = ReturnType<typeof useMutation<DestroyStatementMutation, DestroyStatementMutationVariables>>[0];

// Derived from the operation's variables rather than imported as the standalone
// `CreateStatementInput`: the indexed access tracks the mutation's actual
// signature, so renaming the argument or tightening the input shape surfaces
// here instead of drifting silently.
export type CreateStatementInput = CreateStatementMutationVariables["statement"];

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
// 2. `config` replaces props, but `build` is SELECTIVE about what becomes a
//    signal --- and that selectivity is the pattern, not a one-off. `config` is
//    still the transport (values arrive from React at construction), but
//    `namedSignals` runs over only the fields that are genuinely reactive
//    state: the two debounce scalars, which a settings UI could retune on a
//    live editor via `useExtensionDependency(PersistenceExtension)`.
//
//    The four collaborators --- `editStatement`, `createStatement`,
//    `destroyStatement`, `interviewUid` --- pass straight through `build`
//    unwrapped. They are injected dependencies, not state: they never take a
//    second value, because the route `useMemo`s the root extension and
//    `LexicalExtensionComposer` memoises the EDITOR on that extension's
//    identity (see the SearchInterviewExtension header). A new Apollo executor
//    identity could reach this extension only through that `useMemo` dep array,
//    and reaching it that way tears the whole document down. A surviving editor
//    is proof the executors did not change --- so there is nothing to react to,
//    and `afterRegistration` calls them directly: `editStatement?.(...)`, no
//    `.peek()`.
//
//    The general rule for any extension's `build`: wrap a field in a signal iff
//    it takes a second value mid-session AND something reacts to that (a
//    `.subscribe` or a `.value` read in a reactive context). Otherwise pass it
//    through. SearchOutput is the all-reactive case; this is the mixed one.
//
// The dirty-set -> statement mapping is unchanged from the plugin: Lexical has no
// operation stream (Slate's model), it hands us dirty node sets per update, so we
// walk each dirty node up to its StatementNode ancestor and dedupe by NodeKey.
// -----------------------------------------------------------------------------
// The full config surface: what the route passes in. `build` splits it --- the
// scalars become signals, the collaborators pass through --- into
// `PersistenceOutput` below.
export interface PersistenceConfig {
    /** Per-statement debounce window for edits/creates, in milliseconds. */
    delay: number;
    /** Debounce window for destroys --- longer, it doubles as the undo grace period. */
    destroyDelay: number;
    /** Apollo's `editStatement` executor. `null` disables edit persistence. */
    editStatement: EditStatementFn | null;
    createStatement: CreateStatementFn | null;
    destroyStatement: DestroyStatementFn | null;
    interviewUid: string;
}

// What `state.getOutput()` yields: reactive scalars as signals, collaborators
// as-is. The `Signal<T>` wrappers here are the ones that earn it --- a live
// settings UI is the only writer, and there is no such thing yet, but the seam
// is cheap to keep. Everything else is a plain injected value.
export interface PersistenceOutput {
    delay: Signal<number>;
    destroyDelay: Signal<number>;
    editStatement: EditStatementFn | null;
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
    //
    // The selective split: `namedSignals` over the two reactive scalars only,
    // spread alongside the four collaborators passed through verbatim. The return
    // is annotated `PersistenceOutput` rather than inferred, so the shape --- two
    // `Signal<number>` and four plain values --- is stated once and checked here.
    build: (_editor, config): PersistenceOutput => ({
        ...namedSignals({ delay: config.delay, destroyDelay: config.destroyDelay }),
        editStatement: config.editStatement,
        createStatement: config.createStatement,
        destroyStatement: config.destroyStatement,
        interviewUid: config.interviewUid,
    }),

    afterRegistration (editor, _config, state) {
        // `delay`/`destroyDelay` are signals --- `.peek()` at each debouncer's
        // construction reads the current window. The rest are plain: the executors
        // are called directly (`editStatement?.(...)`), `interviewUid` is a string.
        const { delay, destroyDelay, editStatement, destroyStatement, createStatement, interviewUid } = state.getOutput();

        const createDebouncedUpdate = (uid: string) =>
            debounce((text: string, startTime: number, endTime: number) => {
                // Called directly --- `editStatement` is a plain injected function,
                // stable for the editor's lifetime (see point 2 of the header).
                editStatement?.({
                    variables: { uid, text, startTime, endTime },
                    onCompleted: data => {
                        console.debug(`Edit completed for statement ${ data.editStatement.statement.uid }:`, data.editStatement);
                    },
                });
                // `delay` IS a signal --- `.peek()` reads the window as of the
                // moment the debouncer is built, without subscribing.
            }, delay.peek());

        const createDebouncedDestroy = (uid: string) =>
            debounce(() => {
                destroyStatement?.({
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

                // `interviewUid` is a plain string --- one interview per editor,
                // fixed at construction.
                const uid = interviewUid;

                // Either the statement vanished between the edit and the debounce
                // firing, or we have no interview to attach it to.
                if (!payload || !uid) {
                    return;
                }

                createStatement?.({
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
