import { $getSelection, $isRangeSelection, COMMAND_PRIORITY_LOW, KEY_ENTER_COMMAND, defineExtension } from "lexical";
import { $getExtensionDependency } from "@lexical/extension";
import { $findMatchingParent } from "@lexical/utils";
import { $isStatementNode } from "./StatementNode";
import { StatementExtension } from "./StatementExtension";

// Caret at the very start/end of a statement, Enter pressed: stamp that edge
// with the video's current position. No config -- see StatementSeekExtension;
// same playhead, same dependency.
//
// Watch: this and TagSplitBoundaryExtension both register KEY_ENTER_COMMAND at
// COMMAND_PRIORITY_LOW. Registration order within a priority decides who wins
// -- preserve dependency ordering at the call site rather than reordering.
export const UpdateTimestampExtension = /* @__PURE__ */ defineExtension({
    dependencies: [StatementExtension],
    name: "@auohp/update-timestamp",
    register: editor =>
        editor.registerCommand(
            KEY_ENTER_COMMAND,
            event => {
                const selection = $getSelection();

                if (!$isRangeSelection(selection) || !selection.isCollapsed()) {
                    return false;
                }

                const anchor = selection.anchor.getNode();
                const statement = $isStatementNode(anchor)
                    ? anchor
                    : $findMatchingParent(anchor, $isStatementNode)!;

                if (!$isStatementNode(statement)) {
                    return false;
                }

                const playhead = $getExtensionDependency(StatementExtension).output;

                if (selection.anchor.offset === 0) {
                    editor.update(() => {
                        statement.setStartTime(playhead.timestamp);
                    });
                    event?.preventDefault();
                    return true;
                }

                if (selection.anchor.offset === anchor.getTextContentSize()) {
                    editor.update(() => {
                        statement.setEndTime(playhead.timestamp);
                    });
                    event?.preventDefault();
                    return true;
                }

                return false;
            },
            COMMAND_PRIORITY_LOW,
        ),
});
