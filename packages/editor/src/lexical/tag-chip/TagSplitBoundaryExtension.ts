import { $getSelection, $isRangeSelection, COMMAND_PRIORITY_LOW, KEY_ENTER_COMMAND, defineExtension } from "lexical";
import { $findMatchingParent } from "@lexical/utils";
import { StatementExtension } from "~/lexical/statement/StatementExtension";
import { TagChipExtension } from "./TagChipExtension";
import { $isTagChipNode } from "./TagChipNode";

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
