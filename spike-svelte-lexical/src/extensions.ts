// -----------------------------------------------------------------------------
// TagChipExtension, ported.
//
// Compare the original (packages/editor/src/lexical/extensions.tsx L226):
//
//   dependencies: [
//       MarkExtension,
//       configExtension(ReactExtension, { decorators: [TagChipPortals] }),
//   ],
//   register: editor => editor.registerCommand(INSERT_TAG_CHIP_COMMAND, ...)
//
// The command handler is IDENTICAL --- `$wrapSelectionInMarkNode` with the
// factory hook is core Lexical + @lexical/mark. Only the decorator dependency
// changes: instead of routing through ReactExtension's `decorators` channel
// (which needs a React root, a context provider, and `createPortal`), the
// Svelte version does the registration directly in `register`, because
// `registerSvelteDecorator` returns a plain teardown --- the exact shape
// `register` already wants.
//
// Note what that removes: there is no SvelteExtension *dependency node* in the
// graph at all. ReactExtension exists to own the React root that portals need
// a parent fiber in. Svelte's `mount()` needs no root, so the analogue is a
// FUNCTION, not an extension. This is the one place the port is strictly
// simpler than the original.
// -----------------------------------------------------------------------------

import {
    $getSelection,
    $isRangeSelection,
    COMMAND_PRIORITY_LOW,
    defineExtension,
    type LexicalEditor,
} from "lexical";
import { $wrapSelectionInMarkNode, MarkExtension } from "@lexical/mark";
import { mergeRegister } from "@lexical/utils";

import { INSERT_TAG_CHIP_COMMAND } from "./commands";
import { $createTagChipNode, TAG_CHIP_BADGE_CLASS, TagChipNode } from "./nodes";
import { registerSvelteDecorator } from "./svelte-extension.svelte";
import TagChip from "./TagChip.svelte";

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

            // The seam.
            registerSvelteDecorator(editor, TagChipNode, {
                component: TagChip as never,
                props: key => ({ nodeKey: key }),
                resolveHost: element =>
                    element.querySelector<HTMLElement>(`:scope > .${TAG_CHIP_BADGE_CLASS}`),
            }),
        ),
});
