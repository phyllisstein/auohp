// The Svelte analogue of `useLexicalComposerContext()`. Svelte's context API
// (`setContext`/`getContext`) is component-tree scoped, which is exactly WRONG
// for a decorator: the decorator is mounted by `mount()` into a detached slot,
// so it has no Svelte parent and therefore no context to inherit.
//
// This is a real, if small, difference from React. `createPortal` renders into
// the CALLING component's tree, so React context flows through a portal for
// free. Svelte's `mount()` starts a brand-new root; context does not cross it.
//
// The fix is unremarkable: pass the editor explicitly. Here it is a module
// singleton because there is exactly one editor in the spike; in the real port
// it would be a prop on the decorator's props object (which `DecoratorSpec.props`
// already supports), or a context passed via mount's `context` option in
// Svelte 5.
import type { LexicalEditor } from "lexical";

let current: LexicalEditor | null = null;

export function setEditor(editor: LexicalEditor) {
    current = editor;
}

export function getEditor(): LexicalEditor {
    if (!current) {
        throw new Error("No editor set");
    }
    return current;
}
