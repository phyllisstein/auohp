import { defineExtension, safeCast } from "lexical";
import { createPlayhead, type Playhead } from "../../playhead.svelte";
import { StatementNode } from "./StatementNode";

export interface StatementExtensionConfig {
    // Read source for insertNewAfter's caption-window split (see StatementNode).
    // The route supplies one playhead per interview via configExtension, per
    // MIGRATION.md's per-instance ownership decision. The default below is a
    // throwaway instance, same as PersistenceConfig's null executors --- it
    // exists only so `config` type-checks, and is never the one an editor
    // actually runs against once step 6 wires the route.
    playhead: Playhead;
}

// Minimal on purpose: registers StatementNode and vends the playhead config
// through to it via $getExtensionDependency. No seek/timestamp behavior here
// -- that is StatementSeekExtension/UpdateTimestampExtension (step 5).
export const StatementExtension = /* @__PURE__ */ defineExtension({
    name: "@auohp/statement",
    nodes: () => [StatementNode],
    config: /* @__PURE__ */ safeCast<StatementExtensionConfig>({
        playhead: createPlayhead(),
    }),
    build: (_editor, config) => config.playhead,
});
