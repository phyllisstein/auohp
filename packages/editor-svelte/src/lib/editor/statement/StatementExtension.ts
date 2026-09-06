import { defineExtension, safeCast } from "lexical";
import type { Playhead } from "../../playhead.svelte";
import { StatementNode } from "./StatementNode";

export interface StatementExtensionConfig {
    // Read source for insertNewAfter's caption-window split (see StatementNode).
    // The route supplies one playhead per interview via configExtension.
    playhead: Playhead | null;
}

// Minimal on purpose: registers StatementNode and vends the playhead config
// through to it via $getExtensionDependency. No seek/timestamp behavior here
// -- that is StatementSeekExtension/UpdateTimestampExtension (step 5).
export const StatementExtension = /* @__PURE__ */ defineExtension({
    name: "@auohp/statement",
    nodes: () => [StatementNode],
    config: /* @__PURE__ */ safeCast<StatementExtensionConfig>({
        playhead: null,
    }),
    build: (_editor, config) => {
        if (!config.playhead) {
            throw new Error("StatementExtension: no playhead configured -- the route must supply one via configExtension");
        }
        return config.playhead;
    },
});
