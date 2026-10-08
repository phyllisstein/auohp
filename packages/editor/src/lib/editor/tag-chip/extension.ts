import {
    $getSelection,
    $isRangeSelection,
    COMMAND_PRIORITY_LOW,
    KEY_ENTER_COMMAND,
    configExtension,
    defineExtension,
} from "lexical";
import { $wrapSelectionInMarkNode, MarkExtension } from "@lexical/mark";
import { ReactExtension } from "@lexical/react/ReactExtension";
import { $findMatchingParent } from "@lexical/utils";
import { StatementExtension } from "../statement";
import { INSERT_TAG_CHIP_COMMAND } from "./commands";
import { TagChipPortals } from "./components";
import { $createTagChipNode, $isTagChipNode, TagChipNode } from "./node";

// -----------------------------------------------------------------------------
// TagChipExtension --- the React-in-editor seam.
//
// This is where the extension model pays off most visibly: the node and the
// command that inserts it are declared together. Under the old model the node
// went in the composer's `initialConfig.nodes` and the command handler went in a
// <TagChipPlugin/> elsewhere in the JSX; nothing tied them together but
// convention and hope.
// -----------------------------------------------------------------------------
export const TagChipExtension = /* @__PURE__ */ defineExtension({
    name: "@auohp/tag-chip",
    nodes: () => [TagChipNode],
    dependencies: [
        MarkExtension,
        configExtension(ReactExtension, { decorators: [TagChipPortals] }),
    ],
    register: editor =>
        editor.registerCommand(
            INSERT_TAG_CHIP_COMMAND,
            id => {
                const selection = $getSelection();
                if (!$isRangeSelection(selection)) {
                    return false;
                }

                // `$wrapSelectionInMarkNode` does the whole selection -> element
                // wrap, including splitting boundary TextNodes. The 4th argument
                // is the factory hook that lets us substitute our subclass for a
                // plain MarkNode --- it receives the accumulated ids, so
                // overlapping tags merge rather than nest.
                $wrapSelectionInMarkNode(selection, false, id, ids => $createTagChipNode(ids));
                return true;
            },
            COMMAND_PRIORITY_LOW,
        ),
});

// -----------------------------------------------------------------------------
// TagSplitBoundaryExtension --- keep proper nouns whole across caption breaks.
//
// StatementNode.insertNewAfter now handles the split itself, so the old
// SplitStatementExtension is gone. What remains is an editorial rule, not a
// structural one: a caption boundary should never fall inside a proper noun.
// Enter halfway through "Larry Kramer" should still split --- just not there.
//
// This has to run before anything mutates. `$removeTextAndSplitBlock` walks up
// splitting nodes until it reaches a block, and a chip is inline
// (INTERNAL_$isBlock requires !isInline()), so it cleaves the chip in two on its
// way to the statement. By the time insertNewAfter is called it is already too
// late to object.
//
// COMMAND_PRIORITY_LOW (1) runs before RichTextExtension's default handler at
// COMMAND_PRIORITY_EDITOR (0) --- the bus dispatches high-to-low. We relocate
// the caret and return `false`, so the default handler proceeds normally; it
// re-reads `$getSelection()` rather than closing over one, so it sees our edit.
// -----------------------------------------------------------------------------
export const TagSplitBoundaryExtension = /* @__PURE__ */ defineExtension({
    dependencies: [StatementExtension, TagChipExtension],
    name: "@auohp/tag-split-boundary",
    register: editor =>
        editor.registerCommand(
            KEY_ENTER_COMMAND,
            event => {
                const selection = $getSelection();

                // Non-collapsed selections replace their content before
                // splitting --- a different problem, left alone for now.
                if (!$isRangeSelection(selection) || !selection.isCollapsed()) {
                    return false;
                }

                const anchor = selection.anchor.getNode();
                const chip = $isTagChipNode(anchor)
                    ? anchor
                    : $findMatchingParent(anchor, $isTagChipNode)!;

                // Caret is not inside a proper noun --- nothing to consolidate.
                if (!$isTagChipNode(chip)) {
                    return false;
                }

                // FIXME: Show UX feedback or make a call on splitting the text
                // before/after the chip. For now, silently bail in all cases.
                event?.preventDefault();
                return true;
            },
            COMMAND_PRIORITY_LOW,
        ),
});
