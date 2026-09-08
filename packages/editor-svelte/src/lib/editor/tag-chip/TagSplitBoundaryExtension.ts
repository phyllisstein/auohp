import { $getSelection, $isRangeSelection, COMMAND_PRIORITY_LOW, KEY_ENTER_COMMAND, defineExtension } from "lexical";
import { $findMatchingParent } from "@lexical/utils";
import { StatementExtension } from "../statement/StatementExtension";
import { TagChipExtension } from "./TagChipExtension";
import { $isTagChipNode } from "./TagChipNode";

// Keep proper nouns whole across caption breaks -- an editorial rule, not a
// structural one. Enter halfway through "Larry Kramer" should still split,
// just not there.
//
// Must run before $removeTextAndSplitBlock (RichTextExtension's default Enter
// handler): a chip is inline, so that handler cleaves it in two on its way up
// to the enclosing block. COMMAND_PRIORITY_LOW (1) beats
// COMMAND_PRIORITY_EDITOR (0) because the command bus dispatches high-to-low.
// We relocate nothing and return false, so the default handler still runs
// after us and re-reads $getSelection() itself.
export const TagSplitBoundaryExtension = /* @__PURE__ */ defineExtension({
    dependencies: [StatementExtension, TagChipExtension],
    name: "@auohp/tag-split-boundary",
    register: editor =>
        editor.registerCommand(
            KEY_ENTER_COMMAND,
            event => {
                const selection = $getSelection();

                // Non-collapsed selections replace their content before
                // splitting -- a different problem, left alone for now.
                if (!$isRangeSelection(selection) || !selection.isCollapsed()) {
                    return false;
                }

                const anchor = selection.anchor.getNode();
                const chip = $isTagChipNode(anchor)
                    ? anchor
                    : $findMatchingParent(anchor, $isTagChipNode);

                // Caret is not inside a proper noun -- nothing to consolidate.
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
