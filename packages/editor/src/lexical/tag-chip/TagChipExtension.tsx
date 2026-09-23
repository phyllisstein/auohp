import { $getSelection, $isRangeSelection, COMMAND_PRIORITY_LOW, configExtension, defineExtension } from "lexical";
import { $wrapSelectionInMarkNode, MarkExtension } from "@lexical/mark";
import { ReactExtension } from "@lexical/react/ReactExtension";
import type { JSX } from "react";
import { useNodeDecorators, type ResolveHost } from "~/lexical/react-decorator";
import { INSERT_TAG_CHIP_COMMAND } from "./commands";
import { TagChip, TagChipStyles } from "./TagChip";
import { $createTagChipNode, TAG_CHIP_BADGE_CLASS, TagChipNode } from "./TagChipNode";

// -----------------------------------------------------------------------------
// TagChipExtension --- the React-in-editor seam.
//
// This is where the extension model pays off most visibly: the node and the
// command that inserts it are declared together. Under the old model the node
// went in the composer's `initialConfig.nodes` and the command handler went in a
// <TagChipPlugin/> elsewhere in the JSX; nothing tied them together but
// convention and hope.
// -----------------------------------------------------------------------------
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


const resolveTagChipHost: ResolveHost = element =>
    element.querySelector<HTMLElement>(`:scope > .${ TAG_CHIP_BADGE_CLASS }`);

// The chips' React faces and styles, portalled into each TagChipNode's unmanaged badge
// through the shared decorator seam (see react-decorator.tsx). Rendered via
// ReactExtension's `decorators` channel, which exists precisely for "JSX inside
// the editor context that is not location-dependent".
export function TagChipPortals (): JSX.Element {
    const portals = useNodeDecorators(TagChipNode, resolveTagChipHost, key => <TagChip nodeKey={ key } />);

    return (
        <>
            <TagChipStyles />
            { portals }
        </>
    );
}
