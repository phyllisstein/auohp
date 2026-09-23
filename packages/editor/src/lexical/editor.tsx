import { $createTextNode, $getRoot, configExtension, defineExtension } from "lexical";
import { HistoryExtension } from "@lexical/history";
import { RichTextExtension } from "@lexical/rich-text";
import { ReactExtension, type EditorChildrenComponentProps } from "@lexical/react/ReactExtension";
import { useExtensionComponent } from "@lexical/react/useExtensionComponent";
import type { JSX } from "react";
import type { TranscriptQuery } from "~/__generated__/queries.gql";
import type { PlayheadModel } from "~/playhead";
import { LatencyExtension } from "~/lexical/latency/LatencyExtension";
import {
    PersistenceExtension,
    type CreateStatementFn,
    type DestroyStatementFn,
    type EditStatementFn,
} from "~/lexical/persistence/PersistenceExtension";
import { SearchBar } from "~/lexical/search-interview/SearchBar";
import { SearchInterviewExtension, type SearchStatementsFn } from "~/lexical/search-interview/SearchInterviewExtension";
import { StatementExtension } from "~/lexical/statement/StatementExtension";
import { $createStatementNode, type StatementNode } from "~/lexical/statement/StatementNode";
import { StatementSeekExtension } from "~/lexical/statement/StatementSeekExtension";
import { StatementStyles } from "~/lexical/statement/StatementStyles";
import { UpdateTimestampExtension } from "~/lexical/statement/UpdateTimestampExtension";
import { TagButton } from "~/lexical/tag-chip/TagButton";
import { TagMarkStyles } from "~/lexical/tag-chip/TagChip";
import { TagChipExtension } from "~/lexical/tag-chip/TagChipExtension";
import { TagSplitBoundaryExtension } from "~/lexical/tag-chip/TagSplitBoundaryExtension";


// -----------------------------------------------------------------------------
// The composition root.
//
// `src/lexical/` is organised by feature, not by technical kind. Each directory
// owns its node classes, its command tokens (co-located with the extension that
// handles them), its `defineExtension` call, its React faces, and its styles:
//
//     statement/          StatementNode, StatementExtension (schema + playhead),
//                         click-to-seek, Enter-at-edge timestamping
//     tag-chip/           TagChipNode, insert command, chip face, split boundary
//     search-interview/   in-editor find/replace: node, extension, bar, matching
//     persistence/        the write path and the synthetic-uid marker
//     latency/            instrumentation
//
// Few things cross feature boundaries, and each crossing is deliberate:
// `StatementNode`/`StatementExtension` (everyone edits statements),
// `SYNTHETIC_UID_MARKER` (the node mints synthetic uids, persistence skips
// them), and the playhead (supplied here, read through StatementExtension's
// output). This file is the only place that knows every feature exists.
// -----------------------------------------------------------------------------

export type TranscriptStatements = TranscriptQuery["interview"]["transcript"]["statements"];

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
// Every option in this interface must be stable for the editor's entire
// lifetime, because the route has to memoise the returned extension and any
// change to it destroys the document. Collaborators (the playhead, the GraphQL
// executors) are fine: they never take a second value. Live data belongs in
// signals, not here.
export interface AuohpEditorOptions {
    statements: TranscriptStatements;
    playhead: PlayheadModel;
    editStatement: EditStatementFn;
    createStatement: CreateStatementFn;
    destroyStatement: DestroyStatementFn;
    searchStatements: SearchStatementsFn;
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
            // `interviewUid` goes to search directly rather than being read off
            // PersistenceExtension's output: the read path should not depend on
            // the write path to learn which interview it is searching.
            configExtension(SearchInterviewExtension, { searchStatements, interviewUid }),
            configExtension(ReactExtension, { EditorChildrenComponent: EditorChrome }),
            HistoryExtension,
            LatencyExtension,
            RichTextExtension,
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

        // Seeded from InitialStateExtension's own `afterRegistration` --- root index
        // 0, so first in that loop, ahead of PersistenceExtension's --- inside an
        // `editor.update()` tagged history-merge whose commit lands a microtask
        // later. That tag, not run order, is what keeps the seed out of
        // PersistenceExtension's write path; see its header.
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

// ReactExtension renders `<>{contentEditable}{children}</>` by default, which
// would put our toolbar below the transcript. Overriding EditorChildrenComponent
// through `configExtension` is how the new model does layout composition --- the
// editor's chrome is configuration of an extension rather than JSX the route
// happens to nest in the right order.
function EditorChrome ({ contentEditable, children }: EditorChildrenComponentProps): JSX.Element {
    const Meter = useExtensionComponent(LatencyExtension);

    return (
        <>
            <StatementStyles />
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
