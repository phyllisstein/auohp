import { $getSelection, $isRangeSelection, COMMAND_PRIORITY_LOW, KEY_ENTER_COMMAND, defineExtension } from "lexical";
import { $findMatchingParent } from "@lexical/utils";
import { playhead } from "~/playhead";
import { StatementExtension } from "./StatementExtension";
import { $isStatementNode } from "./StatementNode";

export const UpdateTimestampExtension = /* @__PURE__ */ defineExtension({
    dependencies: [StatementExtension],
    name: "@auohp/update-timestamp",
    register: editor =>
        editor.registerCommand(
            KEY_ENTER_COMMAND,
            event => {
                console.log("UpdateTimestampExtension: KEY_ENTER_COMMAND fired");
                const selection = $getSelection();

                if (!$isRangeSelection(selection) || !selection.isCollapsed()) {
                    return false;
                }

                const anchor = selection.anchor.getNode();
                const statement = $isStatementNode(anchor)
                    ? anchor!
                    : $findMatchingParent(anchor, $isStatementNode)!;

                if (selection.anchor.offset === 0) {
                    console.log("UpdateTimestampExtension: caret at start of statement, updating startTime");
                    editor.update(() => {
                        const currentTime = playhead.timestamp.peek();
                        statement.setStartTime(currentTime);
                    });
                    event?.preventDefault();
                    return true;
                }

                if (selection.anchor.offset === anchor.getTextContentSize()) {
                    console.log("UpdateTimestampExtension: caret at end of statement, updating endTime");
                    editor.update(() => {
                        const currentTime = playhead.timestamp.peek();
                        statement.setEndTime(currentTime);
                    });
                    event?.preventDefault();
                    return true;
                }

                return false;
            },
            COMMAND_PRIORITY_LOW,
        ),
});
