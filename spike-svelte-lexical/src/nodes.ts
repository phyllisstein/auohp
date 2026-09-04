// Ported near-verbatim from packages/editor/src/lexical/nodes.tsx (TagChipNode).
// The ONLY thing removed is the React `TagChip` component and its JSX; the node
// class itself is framework-agnostic and needed no changes at all. That is the
// headline of this file: `createDOM`, `getDOMSlot`, `$config`, `afterCloneFrom`,
// `updateFromJSON` are all plain DOM + Lexical, and a React->Svelte port does
// not touch a line of them.

import { addClassNamesToElement } from "@lexical/utils";
import { MarkNode } from "@lexical/mark";
import {
    $applyNodeReplacement,
    setDOMUnmanaged,
    type EditorConfig,
    type ElementNode,
    type LexicalNode,
    type LexicalUpdateJSON,
    type NodeKey,
    type RangeSelection,
    type SerializedElementNode,
    type Spread,
} from "lexical";

const NO_IDS: readonly string[] = [];

type SerializedTagChipNode = Spread<{ ids: string[] }, SerializedElementNode>;

export const TAG_CHIP_BADGE_CLASS = "auohp-tag-chip__badge";

export class TagChipNode extends MarkNode {
    static clone(node: TagChipNode): TagChipNode {
        return new TagChipNode(node.__ids, node.__key);
    }

    constructor(ids: readonly string[] = NO_IDS, key?: NodeKey) {
        super(ids as string[], key);
        this.__ids = ids as string[];
    }

    $config() {
        return this.config("tag-chip", { extends: MarkNode });
    }

    afterCloneFrom(prevNode: this): void {
        super.afterCloneFrom(prevNode);
        this.__ids = prevNode.__ids;
    }

    updateFromJSON(serializedNode: LexicalUpdateJSON<SerializedTagChipNode>): this {
        return super.updateFromJSON(serializedNode).setIDs(serializedNode.ids);
    }

    insertNewAfter(selection: RangeSelection, restoreSelection: boolean = true): ElementNode | null {
        const tagChipNode = $createTagChipNode(this.__ids);
        this.insertAfter(tagChipNode, restoreSelection);
        return tagChipNode;
    }

    // The badge is real chrome, not a pseudo-element: `setDOMUnmanaged` makes
    // Lexical's mutation-attribution up-walk terminate here, so a FOREIGN
    // framework may render arbitrarily deep inside it without the observer
    // evicting the DOM. Nothing about that mechanism is React-specific --- it
    // is the reason a Svelte decorator is possible at all.
    createDOM(config: EditorConfig): HTMLElement {
        const mark = super.createDOM(config);
        addClassNamesToElement(mark, "auohp-tag-chip");

        const badge = document.createElement("span");
        badge.className = TAG_CHIP_BADGE_CLASS;
        setDOMUnmanaged(badge);
        mark.prepend(badge);

        return mark;
    }

    getDOMSlot(element: HTMLElement) {
        const badge = element.querySelector<HTMLElement>(`:scope > .${TAG_CHIP_BADGE_CLASS}`);
        return badge ? super.getDOMSlot(element).withAfter(badge) : super.getDOMSlot(element);
    }

    collapseAtStart(): true {
        return true;
    }

    getIDs(): string[] {
        return [...this.getLatest().__ids];
    }
}

export function $createTagChipNode(ids: readonly string[] = NO_IDS, key?: NodeKey): TagChipNode {
    return $applyNodeReplacement(new TagChipNode(ids, key));
}

export function $isTagChipNode(node?: LexicalNode | null): node is TagChipNode {
    return node instanceof TagChipNode;
}
