import { $createTextNode, $getRoot, configExtension, defineExtension } from "lexical";
import { HistoryExtension } from "@lexical/history";
import { RichTextExtension } from "@lexical/rich-text";
import type { Playhead } from "../playhead.svelte";
import { StatementExtension } from "./statement/StatementExtension";
import { StatementSeekExtension } from "./statement/StatementSeekExtension";
import { UpdateTimestampExtension } from "./statement/UpdateTimestampExtension";
import { $createStatementNode, StatementNode } from "./statement/StatementNode";
import { TagChipExtension } from "./tag-chip/TagChipExtension";
import { TagSplitBoundaryExtension } from "./tag-chip/TagSplitBoundaryExtension";
import { LatencyExtension } from "./latency/LatencyExtension.svelte";
import {
    PersistenceExtension,
    type CreateStatementFn,
    type DestroyStatementFn,
    type EditStatementFn,
} from "./persistence/PersistenceExtension";
import { SearchInterviewExtension, type SearchStatementsFn } from "./search-interview/SearchInterviewExtension";

// The composition root: everything the source's JSX spelled out by hand
// (namespace, node registration, onError, RichTextPlugin, HistoryPlugin, the
// five bespoke plugins) folds into this one extension. No ReactExtension, no
// EditorChildrenComponent -- the route writes the chrome as ordinary markup
// around the contenteditable (PLAN.md sec 2).
export interface TranscriptStatement {
    uid: string;
    text: string;
    startTime: number | null;
    endTime: number | null;
}

export interface AuohpEditorOptions {
    statements: readonly TranscriptStatement[];
    playhead: Playhead;
    editStatement: EditStatementFn | null;
    createStatement: CreateStatementFn | null;
    destroyStatement: DestroyStatementFn | null;
    searchStatements: SearchStatementsFn | null;
    interviewUid: string;
}

export function defineAuohpEditorExtension ({
    statements,
    playhead,
    editStatement,
    createStatement,
    destroyStatement,
    searchStatements,
    interviewUid,
}: AuohpEditorOptions) {
    return defineExtension({
        dependencies: [
            configExtension(StatementExtension, { playhead }),
            configExtension(PersistenceExtension, { editStatement, createStatement, destroyStatement, interviewUid }),
            configExtension(SearchInterviewExtension, { searchStatements }),
            HistoryExtension,
            LatencyExtension,
            RichTextExtension,
            StatementSeekExtension,
            TagChipExtension,
            TagSplitBoundaryExtension,
            UpdateTimestampExtension,
        ],
        name: "@auohp/editor",
        namespace: "auohp-editor",
        onError: (error: Error) => {
            throw error;
        },

        // Seeds from InitialStateExtension's own afterRegistration -- as root
        // index 0, it runs first among all afterRegistration hooks, so this
        // fires before PersistenceExtension's. Its editor.update() is tagged
        // history-merge, and that tag (not run order relative to register) is
        // what shields PersistenceExtension's update listener from seeing the
        // seed as user edits -- see PersistenceExtension's
        // afterRegistration-not-register comment.
        $initialEditorState () {
            const root = $getRoot();
            root.clear();

            const appendables: StatementNode[] = [];
            for (const statement of statements) {
                const statementNode = $createStatementNode(
                    statement.uid,
                    statement.startTime,
                    statement.endTime,
                );
                statementNode.append($createTextNode(statement.text));
                appendables.push(statementNode);
            }
            root.append(...appendables);

            return root;
        },
    });
}
