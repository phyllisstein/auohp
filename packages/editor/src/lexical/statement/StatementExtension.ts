import { defineExtension, safeCast } from "lexical";
import type { PlayheadModel } from "~/playhead";
import { StatementNode } from "./StatementNode";

export interface StatementExtensionConfig {
    /**
     * The video position this editor's statements are timed against. Read by
     * StatementNode.insertNewAfter, StatementSeekExtension and
     * UpdateTimestampExtension through this extension's output.
     */
    playhead: PlayheadModel | null;
}

// -----------------------------------------------------------------------------
// StatementExtension --- schema, plus the playhead the schema needs.
//
// It contributes StatementNode and vends the configured playhead as its output.
// The behavioural extensions `dependencies: [StatementExtension]`, which both
// registers the node and documents the coupling; the builder merges the graph,
// so a node is registered once no matter how many extensions ask for it.
//
// The playhead travels through config rather than a module import because
// StatementNode cannot receive it any other way: Lexical constructs and calls
// nodes itself, so there is no constructor call site to inject through. The node
// resolves the current editor's instance of this extension with
// `$getExtensionDependency` instead --- per-editor by construction.
//
// `null` here is a sentinel, not a mode. A missing playhead would make every
// split write `startTime: 0`, so `build` refuses to construct the editor at all
// rather than letting a read site discover the omission. `build`'s return is
// exactly `.output`, so nothing can reach the playhead without passing the check.
// -----------------------------------------------------------------------------
export const StatementExtension = /* @__PURE__ */ defineExtension({
    name: "@auohp/statement",
    nodes: () => [StatementNode],
    config: /* @__PURE__ */ safeCast<StatementExtensionConfig>({
        playhead: null,
    }),
    build: (_editor, config): PlayheadModel => {
        if (!config.playhead) {
            throw new Error("StatementExtension: no playhead configured --- supply one via configExtension");
        }
        return config.playhead;
    },
});
