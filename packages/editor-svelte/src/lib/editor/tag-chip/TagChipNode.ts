import {
    $applyNodeReplacement,
    addClassNamesToElement,
    ElementNode,
    setDOMUnmanaged,
    type EditorConfig,
    type LexicalNode,
    type LexicalUpdateJSON,
    type NodeKey,
    type RangeSelection,
    type SerializedElementNode,
    type Spread,
} from "lexical";
import { MarkNode } from "@lexical/mark";

const NO_IDS: readonly string[] = [];

// -----------------------------------------------------------------------------
// TagChipNode --- a decorator host, the hard way.
//
// The obvious move is a DecoratorNode, whose `decorate()` renders content at the
// node's own position. We can't use it. A DecoratorNode is a LEAF:
// `LexicalNode.getTextContent()` returns "" for one, and the persistence
// extension ships `statement.getTextContent()` to the server as the
// authoritative Statement.text. A decorator chip would silently delete the
// words it was tagging.
//
// So the chip extends MarkNode (hence ElementNode) and its children are the
// tagged text --- transparent to getTextContent, copy/paste, and search. That
// costs us `decorate()`, since only DecoratorNodes have one. The Svelte
// decorator seam (step 4) gets in the other door instead: createDOM builds an
// unmanaged badge span, and the seam mounts a component into it, driven by a
// mutation listener. Pull becomes push.
//
// MarkNode also brings semantics we would otherwise hand-roll: __ids with
// overlap merging, canInsertTextBefore/After() === false and canBeEmpty() ===
// false (so the chip is already sealed at its boundaries), and isInline() ===
// true.
// -----------------------------------------------------------------------------
type SerializedTagChipNode = Spread<{ ids: string[] }, SerializedElementNode>;

// Shared by createDOM (which writes it) and getDOMSlot (which must re-find the
// element it names) --- keeping them in sync by construction rather than by two
// matching string literals.
export const TAG_CHIP_BADGE_CLASS = "auohp-tag-chip__badge";

export class TagChipNode extends MarkNode {
    static clone (node: TagChipNode): TagChipNode {
        return new TagChipNode(node.__ids, node.__key);
    }

    constructor (
        ids: readonly string[] = NO_IDS,
        key?: NodeKey,
    ) {
        super(ids, key);
        this.__ids = ids;
    }

    $config () {
        return this.config("tag-chip", {
            extends: MarkNode,
        });
    }

    afterCloneFrom (prevNode: this): void {
        super.afterCloneFrom(prevNode);
        this.__ids = prevNode.__ids;
    }

    updateFromJSON (serializedNode: LexicalUpdateJSON<SerializedTagChipNode>): this {
        return super.updateFromJSON(serializedNode).setIDs(serializedNode.ids);
    }

    insertNewAfter (selection: RangeSelection, restoreSelection: boolean = true): ElementNode | null {
        const tagChipNode = $createTagChipNode(this.__ids);
        this.insertAfter(tagChipNode, restoreSelection);
        return tagChipNode;
    }

    // Defer to MarkNode for the <mark> itself: it applies `config.theme.mark`
    // AND `config.theme.markOverlap` when __ids.length > 1, which is free
    // multi-tag styling we would lose by hand-rolling the element. We only add
    // our own class and the badge host on top.
    //
    // The badge is real chrome, not a pseudo-element: `setDOMUnmanaged` makes
    // Lexical's mutation-attribution up-walk terminate here, so the decorator
    // may render arbitrarily deep inside it without the observer evicting the
    // DOM as foreign.
    createDOM (config: EditorConfig): HTMLElement {
        const mark = super.createDOM(config);
        addClassNamesToElement(mark, "auohp-tag-chip");

        const badge = document.createElement("span");
        badge.className = TAG_CHIP_BADGE_CLASS;
        setDOMUnmanaged(badge);
        mark.prepend(badge);

        return mark;
    }

    // Tell the reconciler its managed range starts AFTER the badge. `after` is a
    // boundary node reference rather than an index, so nothing needs
    // recomputing when children churn --- and `resolveChildIndex` then handles
    // DOM-caret -> lexical-offset mapping for free.
    //
    // Re-find the badge from `element` rather than closing over the one
    // createDOM built: getDOMSlot runs against the latest node version, and
    // clone() mints new instances constantly.
    getDOMSlot (element: HTMLElement) {
        const badge = element.querySelector<HTMLElement>(`:scope > .${ TAG_CHIP_BADGE_CLASS }`);
        return badge ? super.getDOMSlot(element).withAfter(badge) : super.getDOMSlot(element);
    }

    // NOTE: no updateDOM override --- MarkNode's own implementation maintains
    // the overlap class as __ids crosses 1 <-> 2, and returns false so our DOM
    // is never rebuilt.

    collapseAtStart (): true {
        return true;
    }

    getIDs (): string[] {
        return [...this.getLatest().__ids];
    }
}

export function $createTagChipNode (ids: readonly string[] = NO_IDS, key?: NodeKey): TagChipNode {
    return $applyNodeReplacement(new TagChipNode(ids, key));
}

export function $isTagChipNode (node?: LexicalNode): node is TagChipNode {
    return node instanceof TagChipNode;
}
