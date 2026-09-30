import { $getNodeByKey, $getRoot, configExtension, defineExtension, safeCast, type NodeKey, type Signal } from "lexical";
import { mergeRegister, $findMatchingParent } from "@lexical/utils";
import { namedSignals } from "@lexical/extension";
import { debounce } from "perfect-debounce";
import { $isStatementNode, $adoptStatementIdentity } from "../statement/node";
import { SYNTHETIC_UID_MARKER } from "../shared";
import type { EditStatementFn, DestroyStatementFn, CreateStatementFn, CreateStatementInput } from "../shared";

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
    dependencies: [],  // Will be injected by composition
    name: "@auohp/persistence",

    build: (_editor, config): PersistenceOutput => ({
        ...namedSignals({ delay: config.delay, destroyDelay: config.destroyDelay }),
        editStatement: config.editStatement,
        createStatement: config.createStatement,
        destroyStatement: config.destroyStatement,
        interviewUid: config.interviewUid,
    }),

    afterRegistration (editor, _config, state) {
        const { delay, destroyDelay, editStatement, destroyStatement, createStatement, interviewUid } = state.getOutput();

        const createDebouncedUpdate = (uid: string) =>
            debounce((text: string, startTime: number, endTime: number) => {
                editStatement?.({
                    variables: { uid, text, startTime, endTime },
                    onCompleted: data => {
                        console.debug(`Edit completed for statement ${data.editStatement.statement.uid}:`, data.editStatement);
                    },
                });
            }, delay.peek());

        const createDebouncedDestroy = (uid: string) =>
            debounce(() => {
                destroyStatement?.({
                    variables: { uid },
                    onCompleted: data => {
                        console.debug(`Destroy completed for statement ${data.destroyStatement.statement.uid}:`, data.destroyStatement);
                    },
                });
            }, destroyDelay.peek());

        const createDebouncedCreate = (_uid: string) =>
            debounce((key: NodeKey) => {
                const payload = editor.read((): CreateStatementInput | null => {
                    const node = $getNodeByKey(key)!;
                    if (!$isStatementNode(node)) {
                        return null;
                    }
                    const startTime = node.getStartTime();
                    const endTime = node.getEndTime();
                    if (startTime === null || endTime === null) {
                        return null;
                    }
                    return { text: node.getTextContent(), startTime, endTime };
                });

                const uid = interviewUid;

                if (!payload || !uid) {
                    return;
                }

                createStatement?.({
                    variables: { statement: payload, interviewUid: uid },
                    onCompleted: data => {
                        console.debug(`Create completed for statement ${data.createStatement.statement.uid}:`, data.createStatement);
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

        const persistCreate = (statementNode: any) => {
            const uid = statementNode.getUid();
            let flush = createDebouncers.get(uid);
            if (!flush) {
                flush = createDebouncedCreate(uid);
                createDebouncers.set(uid, flush);
            }
            flush(statementNode.getKey());
        };

        const unregister = mergeRegister(
            editor.registerUpdateListener(
                ({ dirtyLeaves, dirtyElements, editorState, tags, mutatedNodes, prevEditorState }) => {
                    console.log(`PersistenceExtension: %o mutations, ${dirtyLeaves?.size} dirty leaves, ${dirtyElements?.size} dirty elements, tags: ${Array.from(tags).join(", ")}`, mutatedNodes);
                    if (tags.has("history-merge")) {
                        return;
                    }

                    if (!mutatedNodes?.size) {
                        return;
                    }

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
                                const pendingDestroy = destroyDebouncers.get(uid);
                                if (pendingDestroy && !uid.includes(SYNTHETIC_UID_MARKER)) {
                                    pendingDestroy.cancel();
                                    destroyDebouncers.delete(uid);
                                    console.log(`PersistenceExtension: statement ${uid} restored before its destroy flushed, cancelling`);
                                    return;
                                }

                                console.log(`PersistenceExtension: statement ${uid} created, persisting`);
                                persistCreate(statement);
                                return;
                            }

                            if (update === "updated") {
                                console.log(`PersistenceExtension: statement ${uid} updated, persisting`);
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

                                if (!$isStatementNode(node)) {
                                    continue;
                                }

                                const uid = node.getUid();

                                if (uid.includes(SYNTHETIC_UID_MARKER)) {
                                    const pendingCreate = createDebouncers.get(uid);
                                    pendingCreate?.cancel();
                                    createDebouncers.delete(uid);

                                    console.log(`PersistenceExtension: synthetic statement ${uid} destroyed before creation, cancelling pending create`);
                                    continue;
                                }

                                createDebouncers.get(uid)?.cancel();
                                createDebouncers.delete(uid);

                                updateDebouncers.get(uid)?.cancel();
                                updateDebouncers.delete(uid);

                                console.log(`PersistenceExtension: statement ${uid} destroyed, persisting`);
                                persistDestroy(uid);
                            }
                        });
                    }
                },
            ),
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
