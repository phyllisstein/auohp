import { $getSelection, $isRangeSelection, COMMAND_PRIORITY_LOW, KEY_ENTER_COMMAND, defineExtension } from "lexical";
import { $findMatchingParent } from "@lexical/utils";
import { StatementExtension } from "./StatementExtension";
import { $isStatementNode } from "./StatementNode";

// Caret at the very start/end of a statement, Enter pressed: stamp that edge
// with the video's current position. The playhead comes off StatementExtension's
// output, the same object StatementNode reads.
//
// This and TagSplitBoundaryExtension both register KEY_ENTER_COMMAND at
// COMMAND_PRIORITY_LOW. Registration order within a priority decides who wins,
// so preserve dependency ordering at the composition root.
export const UpdateTimestampExtension = /* @__PURE__ */ defineExtension({
    dependencies: [StatementExtension],
    name: "@auohp/update-timestamp",
    register: (editor, _config, state) => {
        const playhead = state.getDependency(StatementExtension).output;

        return editor.registerCommand(
            KEY_ENTER_COMMAND,
            event => {
                const selection = $getSelection();

                if (!$isRangeSelection(selection) || !selection.isCollapsed()) {
                    return false;
                }

                const anchor = selection.anchor.getNode();
                const statement = $isStatementNode(anchor)
                    ? anchor
                    : $findMatchingParent(anchor, $isStatementNode);

                // A caret outside any statement (an element selection on root,
                // say) has no caption window to stamp.
                if (!$isStatementNode(statement)) {
                    return false;
                }

                if (selection.anchor.offset === 0) {
                    editor.update(() => {
                        statement.setStartTime(playhead.timestamp.peek());
                    });
                    event?.preventDefault();
                    return true;
                }

                if (selection.anchor.offset === anchor.getTextContentSize()) {
                    editor.update(() => {
                        statement.setEndTime(playhead.timestamp.peek());
                    });
                    event?.preventDefault();
                    return true;
                }

                return false;
            },
            COMMAND_PRIORITY_LOW,
        );
    },
});
