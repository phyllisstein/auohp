import { $getRoot, COMMAND_PRIORITY_LOW, defineExtension } from "lexical";
import { $getExtensionDependency } from "@lexical/extension";
import { mergeRegister } from "@lexical/utils";
import { SEEK_VIDEO_COMMAND } from "./commands";
import { $isStatementNode, STATEMENT_CHROME_CLASS, STATEMENT_NODE_CLASS } from "./StatementNode";
import { StatementExtension } from "./StatementExtension";

// Click-to-seek, driven by the chrome. No config of its own -- the playhead
// comes off StatementExtension's dependency output (same object StatementNode
// reads), so there is exactly one place a route wires a playhead in.
export const StatementSeekExtension = /* @__PURE__ */ defineExtension({
    dependencies: [StatementExtension],
    name: "@auohp/statement-seek",

    register (editor) {
        // Hoisted out of the root listener: registerRootListener fires with
        // (nextRoot, prevRoot) on every root change, and removal needs the
        // SAME function reference -- a handler defined inline would be a
        // fresh closure each time and could never be detached.
        const onClick = (event: MouseEvent) => {
            const target = event.target;
            if (!(target instanceof Element)) {
                return;
            }

            // Only the chrome column seeks; a click in editable content is
            // caret placement and must stay inert. The chrome is
            // setDOMUnmanaged + contentEditable=false, so it's outside
            // Lexical's selection machinery entirely -- this listener is the
            // only door to it, not a fallback.
            const chrome = target.closest(`.${ STATEMENT_CHROME_CLASS }`);
            if (!chrome) {
                return;
            }

            const uid = chrome.closest(`.${ STATEMENT_NODE_CLASS }`)?.getAttribute("data-uid");
            if (!uid) {
                return;
            }

            editor.dispatchCommand(SEEK_VIDEO_COMMAND, uid);
        };

        return mergeRegister(
            editor.registerCommand(
                SEEK_VIDEO_COMMAND,
                uid => {
                    // The uid identifies the statement; timing is read live
                    // off the node, so a seek always targets where the
                    // statement is now.
                    const startTime = editor.read(() => {
                        const statement = $getRoot()
                            .getChildren()
                            .find(node => $isStatementNode(node) && node.getUid() === uid);

                        return $isStatementNode(statement) ? statement.getStartTime() : null;
                    });

                    // Non-media statements legitimately have no timing;
                    // seeking to 0 would be worse than not seeking.
                    if (startTime == null) {
                        return false;
                    }

                    const playhead = $getExtensionDependency(StatementExtension).output;
                    playhead.seek = startTime;

                    return true;
                },
                COMMAND_PRIORITY_LOW,
            ),

            editor.registerRootListener((rootElement, prevRootElement) => {
                prevRootElement?.removeEventListener("click", onClick);
                rootElement?.addEventListener("click", onClick);
            }),
        );
    },
});
