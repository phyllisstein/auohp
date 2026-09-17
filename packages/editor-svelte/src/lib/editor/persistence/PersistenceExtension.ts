import { $findMatchingParent, mergeRegister } from "@lexical/utils";
import { $getNodeByKey, defineExtension, safeCast, type NodeKey } from "lexical";
import { debounce } from "perfect-debounce";
import { $adoptStatementIdentity, $isStatementNode, StatementNode } from "../statement/StatementNode";
import { SYNTHETIC_UID_MARKER } from "./synthetic-uid";

// Executor contracts: the extension's boundary with whoever wires it, spelled
// as plain structural types rather than imported from generated GraphQL
// operation types. The documents and codegen belong with the commit that
// actually calls the API (step 6's route, or D) -- keeping persistence/
// independent of the GraphQL library entirely is also PLAN.md sec 3.4's
// stated goal for this extension.
//
// A plain async function closing over the client is the natural urql shape --
// Apollo's { variables, onCompleted } call convention goes away, and
// onCompleted's body becomes code after the await.
export interface CreateStatementInput {
    text: string;
    startTime: number;
    endTime: number;
}

export type EditStatementFn = (variables: {
    uid: string;
    text: string;
    startTime: number;
    endTime: number;
}) => Promise<unknown>;

export type CreateStatementFn = (variables: {
    statement: CreateStatementInput;
    interviewUid: string;
}) => Promise<{ data?: { createStatement: { statement: { uid: string; startTime: number | null; endTime: number | null } } } }>;

export type DestroyStatementFn = (variables: { uid: string }) => Promise<unknown>;

// The full config surface: what the route passes in.
export interface PersistenceConfig {
    /** Per-statement debounce window for edits/creates, in milliseconds. */
    delay: number;
    /** Debounce window for destroys -- longer, it doubles as the undo grace period. */
    destroyDelay: number;
    /** `null` disables edit persistence -- a real mode, always read via `?.`. */
    editStatement: EditStatementFn | null;
    createStatement: CreateStatementFn | null;
    destroyStatement: DestroyStatementFn | null;
    interviewUid: string;
}

// delay/destroyDelay are plain numbers, not a Signal/$state seam. Both are
// read exactly once per debouncer, at construction (see createDebouncedUpdate
// below) -- no debouncer already built re-reads a later value, so a live
// settings UI retuning either mid-session would change nothing today. The
// general rule (wrap iff something takes a second value AND something reacts)
// fails on the second half here; the other four fields are injected
// collaborators, stable for the editor's lifetime, and pass through as-is.
export interface PersistenceOutput {
    delay: number;
    destroyDelay: number;
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
    name: "@auohp/persistence",

    // `build` must be declared before `afterRegistration` -- TypeScript infers
    // this object literal's members in source order, so state.getOutput()'s
    // return type only resolves for members declared after build; otherwise
    // it's `unknown`.
    build: (_editor, config): PersistenceOutput => ({
        delay: config.delay,
        destroyDelay: config.destroyDelay,
        editStatement: config.editStatement,
        createStatement: config.createStatement,
        destroyStatement: config.destroyStatement,
        interviewUid: config.interviewUid,
    }),

    // Not `register`. InitialStateExtension seeds via editor.update(), whose
    // commit is deferred to a microtask, while LexicalBuilder.registerEditor
    // runs its registration loops synchronously -- so the seed's dirty-node
    // wave lands after every `register` has already returned, and an
    // unguarded update listener sees all of it as spurious mutations.
    // `afterRegistration` runs after that microtask has flushed.
    afterRegistration (editor, _config, state) {
        const { delay, destroyDelay, editStatement, destroyStatement, createStatement, interviewUid } = state.getOutput();

        const createDebouncedUpdate = (uid: string) =>
            debounce(async (text: string, startTime: number, endTime: number) => {
                await editStatement?.({ uid, text, startTime, endTime });
            }, delay);

        const createDebouncedDestroy = (uid: string) =>
            debounce(async () => {
                await destroyStatement?.({ uid });
            }, destroyDelay);

        // Takes a NodeKey, never a StatementNode. A node object is a snapshot
        // of one EditorState; this fires from a timer and resolves after a
        // network round trip, so any captured node is stale twice over. The
        // key is the only identity stable across states -- $getNodeByKey
        // re-resolves it against whichever state is active when needed.
        const createDebouncedCreate = (_uid: string) =>
            debounce(async (key: NodeKey) => {
                const payload = editor.read((): CreateStatementInput | null => {
                    const node = $getNodeByKey(key);
                    if (!node || !$isStatementNode(node)) {
                        return null;
                    }
                    const startTime = node.getStartTime();
                    const endTime = node.getEndTime();
                    return { text: node.getTextContent(), startTime, endTime };
                });

                // Either the statement vanished between the edit and the
                // debounce firing, or we have no interview to attach it to.
                if (!payload || !interviewUid) {
                    return;
                }

                // Apollo's onCompleted swallowed a rejected promise; an
                // await continuation does not -- an unhandled error here
                // would otherwise vanish as an unhandled rejection. This is
                // a genuine error path, so it survives the no-logging rule.
                try {
                    const result = await createStatement?.({ statement: payload, interviewUid });
                    const created = result?.data?.createStatement.statement;
                    if (!created) {
                        return;
                    }

                    editor.update(() => {
                        const node = $getNodeByKey(key);
                        if (!node || !$isStatementNode(node)) {
                            return;
                        }
                        $adoptStatementIdentity(node, created);
                    }, { tag: "history-merge" });
                } catch (error) {
                    console.error(`Failed to create statement for key ${ key }:`, error);
                }
            }, delay);

        // One debouncer per statement uid, created lazily on first edit. A
        // single shared debouncer would let fast edits across two statements
        // cancel each other's save -- a latent data-loss bug this shape
        // cannot have.
        //
        // One map per operation, though, not one keyed by uid alone. A
        // shared map would put create/update/destroy for the same statement
        // in the same slot, so a statement edited before its create had
        // flushed would find the create debouncer under its uid and invoke
        // it with the update's arguments -- a NodeKey parameter receiving a
        // text string.
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
            // Hand over the key, not the node -- see createDebouncedCreate.
            flush(statementNode.getKey());
        };

        const unregister = mergeRegister(
            // `mutatedNodes` below is editor-global, not per-node-class: Lexical
            // only populates it once at least one mutation listener exists for
            // that class (Lexical.dev.mjs setMutatedNode). Without this
            // registration, persistence would silently stop saving whenever no
            // other extension happens to register a StatementNode mutation
            // listener -- this no-op exists purely to make that precondition
            // ours instead of a borrowed side effect of TagChip/SearchInterview.
            editor.registerMutationListener(StatementNode, () => {}),
            editor.registerUpdateListener(({ tags, mutatedNodes, editorState, prevEditorState }) => {
                if (tags.has("history-merge")) {
                    return;
                }

                if (!mutatedNodes?.size) {
                    return;
                }

                // A destroyed node cannot be resolved against `editorState` --
                // being absent from it is what "destroyed" means. But
                // Lexical's states are persistent data structures: the
                // previous tree is intact and structurally shared, so the
                // node is still fully readable in `prevEditorState` under the
                // same key -- that's where its uid, the only identifier the
                // server knows, survives.
                //
                // Hence two passes over two states rather than one. `read()`
                // installs its state as the active one for the callback's
                // duration, so a key can only be resolved from inside the
                // pass for the state that contains it.
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
                            : $findMatchingParent(node, $isStatementNode);
                        if (!$isStatementNode(statement) || seen.has(statement.getKey())) {
                            return;
                        }
                        seen.add(statement.getKey());

                        const uid = statement.getUid();

                        if (update === "created") {
                            // "Created" is also what an undone deletion looks
                            // like: the statement reappears with the real uid
                            // it already had on the server. Sending
                            // createStatement for it would mint a duplicate
                            // row while the queued destroy went ahead and
                            // removed the original. A non-synthetic uid is
                            // exactly the signal this row already exists
                            // server-side, so cancel the pending destroy and
                            // treat the resurrection as a no-op.
                            const pendingDestroy = destroyDebouncers.get(uid);
                            if (pendingDestroy && !uid.includes(SYNTHETIC_UID_MARKER)) {
                                pendingDestroy.cancel();
                                destroyDebouncers.delete(uid);
                                return;
                            }

                            persistCreate(statement);
                            return;
                        }

                        if (update === "updated") {
                            const text = statement.getTextContent();
                            const startTime = statement.getStartTime();
                            const endTime = statement.getEndTime();
                            persistUpdate(uid, text, startTime, endTime);
                        }
                    };

                    for (const [, val] of mutatedNodes.entries()) {
                        for (const [key, status] of val.entries()) {
                            collect(key, status);
                        }
                    }
                });

                if (destroyedKeys.length) {
                    prevEditorState.read(() => {
                        for (const key of destroyedKeys) {
                            const node = $getNodeByKey(key);

                            // Only statements are persisted, and -- unlike the
                            // pass above -- we deliberately do NOT walk up to
                            // a parent statement. Deleting a word destroys
                            // TextNodes inside a statement that is still very
                            // much alive; that arrives separately as an
                            // "updated" mutation on the statement itself.
                            // Treating a destroyed child as a destroyed
                            // statement would delete the row the user was
                            // merely editing.
                            if (!node || !$isStatementNode(node)) {
                                continue;
                            }

                            const uid = node.getUid();

                            // A statement born of a split carries a synthetic
                            // uid until createStatement answers with a real
                            // one. Two cases, opposite handling:
                            if (uid.includes(SYNTHETIC_UID_MARKER)) {
                                // The server has never heard of this
                                // statement -- destroyStatement would 404.
                                // Cancelling the pending create is the whole
                                // of the work: no create, no destroy needed.
                                const pendingCreate = createDebouncers.get(uid);
                                pendingCreate?.cancel();
                                createDebouncers.delete(uid);
                                continue;
                            }

                            // A real uid: seeded from the server, or adopted
                            // by $adoptStatementIdentity when a create
                            // completed. Cancel any pending create anyway --
                            // harmless if absent, closes the window where an
                            // unflushed create races the destroy.
                            createDebouncers.get(uid)?.cancel();
                            createDebouncers.delete(uid);

                            // Any queued edit is moot once the row is going away.
                            updateDebouncers.get(uid)?.cancel();
                            updateDebouncers.delete(uid);

                            persistDestroy(uid);
                        }
                    });
                }
            }),
        );

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
