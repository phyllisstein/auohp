import { defineExtension } from "lexical";
import { StatementNode } from "./StatementNode";

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
