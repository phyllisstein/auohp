import type { JSX } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { Button } from "@react-spectrum/s2/Button";
import { INSERT_TAG_CHIP_COMMAND } from "./commands";

// A trivial toolbar affordance that dispatches the typed insert command ---
// demonstrating an out-of-editor React control mutating EditorState, which then
// renders back through a React DecoratorNode. `useLexicalComposerContext` is not
// deprecated: it remains the sanctioned way for a component that genuinely
// renders something to reach the editor (useExtensionComponent is built on it).
// What was deprecated is using it as a back door for behaviour-only components.
export function TagButton (): JSX.Element {
    const [editor] = useLexicalComposerContext();

    // The payload is the mark ID. Deferred decision: a throwaway uid for now, so
    // each chip is at least distinct. When entity resolution lands, this becomes
    // the graph uid of the Person/Organization being mentioned --- MarkNode's
    // __ids then genuinely means "this range mentions these entities", and
    // $getMarkIDs answers that question directly. No signature change needed.
    return (
        <Button
            id="insert-tag-chip"
            type="button"
            onPress={ () => editor.dispatchCommand(INSERT_TAG_CHIP_COMMAND, crypto.randomUUID()) }>
            Insert #person chip
        </Button>
    );
}
