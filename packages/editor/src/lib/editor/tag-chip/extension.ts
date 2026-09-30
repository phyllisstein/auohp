import { COMMAND_PRIORITY_LOW, KEY_ENTER_COMMAND, $getSelection, $isRangeSelection, configExtension, createCommand, defineExtension, type LexicalCommand } from "lexical";
import { MarkExtension, $wrapSelectionInMarkNode, $unwrapMarkNode, $findMatchingParent } from "@lexical/mark";
import { ReactExtension } from "@lexical/react/ReactExtension";
import { mergeRegister } from "@lexical/utils";
import { $isTagChipNode, $createTagChipNode, TagChipNode } from "./node";
import { TagChipPortals } from "./component";

export const INSERT_TAG_CHIP_COMMAND: LexicalCommand<string> = createCommand("INSERT_TAG_CHIP_COMMAND");

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
