import { type JSX } from "react";
import { $createTextNode, $getRoot, configExtension, defineExtension } from "lexical";
import { HistoryExtension } from "@lexical/history";
import { ReactExtension, type EditorChildrenComponentProps } from "@lexical/react/ReactExtension";
import { useExtensionComponent } from "@lexical/react/useExtensionComponent";
import { RichTextExtension } from "@lexical/rich-text";
import { LatencyExtension } from "./latency";
import { PersistenceExtension } from "./persistence";
import { SearchBar, SearchInterviewExtension, SearchResultPortals } from "./search-interview";
import { type CreateStatementFn, type DestroyStatementFn, type EditStatementFn, type TranscriptStatements } from "./shared";
import {
    $createStatementNode,
    StatementExtension,
    StatementSeekExtension,
    UpdateTimestampExtension,
    type StatementNode,
} from "./statement";
import { TagButton, TagChipExtension, TagMarkStyles, TagSplitBoundaryExtension } from "./tag-chip";

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

export function defineSearchResultsExtension () {
    return defineExtension({
        dependencies: [
            SearchInterviewExtension,
            configExtension(ReactExtension, { decorators: [SearchResultPortals] }),
            HistoryExtension,
            RichTextExtension,
            PersistenceExtension,
        ],
        name: "@auohp/search-results",
        namespace: "auohp-lexical-spike",
        onError: (error: Error) => {
            throw error;
        },
    });
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
