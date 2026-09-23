import { $getRoot, COMMAND_PRIORITY_LOW, defineExtension } from "lexical";
import { mergeRegister } from "@lexical/utils";
import { SEEK_VIDEO_COMMAND } from "./commands";
import { StatementExtension } from "./StatementExtension";
import { $isStatementNode, STATEMENT_CHROME_CLASS, STATEMENT_NODE_CLASS } from "./StatementNode";

// -----------------------------------------------------------------------------
// StatementSeekExtension --- click-to-seek, driven by the chrome.
//
// This used to hang off SELECTION_CHANGE_COMMAND, which made *any* arrival in a
// statement seek the video: typing, arrow keys, clicking to place a caret mid-
// sentence. Editing a caption would yank the playhead out from under the editor.
// Seeking is a deliberate act and deserves a deliberate gesture, so the trigger
// is now a click on the timestamp chrome specifically --- and the command carries
// the target uid rather than being inferred from wherever the caret happens to be.
//
// Two mechanisms are worth noting:
//
// 1. The chrome is `setDOMUnmanaged` + contentEditable=false (see StatementNode.
//    createDOM), so Lexical disclaims it entirely --- no selection lands there and
//    no Lexical event machinery reaches it. A plain DOM listener is not a fallback
//    here, it is the only door.
//
// 2. The listener is DELEGATED from the editor's root element rather than attached
//    per-node in createDOM. `createDOM` receives only an EditorConfig and has no
//    editor to dispatch against, so a per-node listener would have to reach for an
//    ambient singleton --- precisely the smell the extension model removes. One
//    root listener also survives reconciliation, which replaces chrome DOM freely.
//
// The command's payload is the uid rather than a timestamp: the handler resolves
// the node and reads its CURRENT startTime, so a seek is always to where the
// statement is now, not to whatever the chrome happened to render when it was
// built.
// -----------------------------------------------------------------------------
export const StatementSeekExtension = /* @__PURE__ */ defineExtension({
    dependencies: [StatementExtension],
    name: "@auohp/statement-seek",

    register (editor, _config, state) {
        const playhead = state.getDependency(StatementExtension).output;

        // Hoisted out of the root listener deliberately. `registerRootListener`
        // fires with (nextRoot, prevRoot) on every root change, and removing a
        // listener requires the SAME function reference --- a handler defined
        // inside the callback would be a fresh closure each time and could never
        // be detached, leaking one listener per root swap.
        const onClick = (event: MouseEvent) => {
            const target = event.target;
            if (!(target instanceof Element)) {
                return;
            }

            // Only the chrome column seeks. A click anywhere in the editable
            // content is caret placement and must stay inert.
            const chrome = target.closest(`.${ STATEMENT_CHROME_CLASS }`);
            if (!chrome) {
                return;
            }

            // `data-uid` is kept in sync by StatementNode.updateDOM, including
            // across the synthetic -> real uid adoption after createStatement.
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
                    // The uid identifies the statement; its timing is read live from
                    // the node. `editor.read()` establishes the active editor state
                    // that `getStartTime()` requires --- the command handler runs
                    // outside any update.
                    //
                    // Statements are direct children of root (see the seeding loop in
                    // InitialStateExtension), so this scan is one level deep. It is
                    // linear in statement count, which is the price of keeping the
                    // command's payload a uid --- the identity a toolbar button or a
                    // search result would dispatch with. The click path below could
                    // resolve its node from the DOM in O(1), but that shortcut is not
                    // available to every caller, and a seek happens at human speed.
                    const startTime = editor.read(() => {
                        const statement = $getRoot()
                            .getChildren()
                            .find(node => $isStatementNode(node) && node.getUid() === uid);

                        return $isStatementNode(statement) ? statement.getStartTime() : null;
                    });

                    // Non-media statements (broadsheet text) legitimately have no
                    // timing. Seeking to 0 would be worse than not seeking at all.
                    if (startTime == null) {
                        return false;
                    }

                    // Seek to the START of the caption window: the user is asking to
                    // hear this statement, which means from its beginning.
                    playhead.seek.value = startTime;
                    console.debug(`Seeking to statement ${ uid } (${ startTime })`);

                    // `true` --- this command is fully handled here, and nothing
                    // below should also act on it.
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
