import {
    $getSelection,
    $isRangeSelection,
    COMMAND_PRIORITY_LOW,
    defineExtension,
    type LexicalEditor,
} from "lexical";
import { $wrapSelectionInMarkNode, MarkExtension } from "@lexical/mark";
import { mergeRegister } from "@lexical/utils";

import { registerSvelteDecorator } from "../svelte-decorator";
import { INSERT_TAG_CHIP_COMMAND } from "./commands";
import { $createTagChipNode, TAG_CHIP_BADGE_CLASS, TagChipNode } from "./TagChipNode";
import TagChip from "./TagChip.svelte";

// The command handler is identical to the React source -- $wrapSelectionInMarkNode
// with the factory hook is core Lexical + @lexical/mark. Only the decorator
// dependency changes: no ReactExtension, no `decorators` channel, no React root.
// registerSvelteDecorator returns a plain teardown, which drops straight into
// mergeRegister alongside the command registration.
export const TagChipExtension = /* @__PURE__ */ defineExtension({
    name: "@auohp/tag-chip",
    nodes: [TagChipNode],
    dependencies: [MarkExtension],
    register: (editor: LexicalEditor) =>
        mergeRegister(
            editor.registerCommand(
                INSERT_TAG_CHIP_COMMAND,
                id => {
                    const selection = $getSelection();
                    if (!$isRangeSelection(selection)) {
                        return false;
                    }
                    $wrapSelectionInMarkNode(selection, false, id, ids => $createTagChipNode(ids));
                    return true;
                },
                COMMAND_PRIORITY_LOW,
            ),

            registerSvelteDecorator(editor, TagChipNode, {
                component: TagChip,
                props: key => ({ editor, nodeKey: key }),
                resolveHost: element =>
                    element.querySelector<HTMLElement>(`:scope > .${ TAG_CHIP_BADGE_CLASS }`),
            }),
        ),
});
