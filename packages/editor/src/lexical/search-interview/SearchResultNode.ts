import {
    $applyNodeReplacement,
    addClassNamesToElement,
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
import { MarkNode } from "@lexical/mark";

const NO_IDS: readonly string[] = [];

type SerializedSearchResultNode = Spread<{ ids: string[] }, SerializedElementNode>;

// Shared by createDOM (which writes it) and getDOMSlot (which must re-find the
// element it names) --- keeping them in sync by construction rather than by
// two matching string literals.
export const SEARCH_RESULT_BADGE_CLASS = "auohp-search-result__badge";

export class SearchResultNode extends MarkNode {
    static clone (node: SearchResultNode): SearchResultNode {
        return new SearchResultNode(node.__ids, node.__key);
    }

    constructor (
        ids: readonly string[] = NO_IDS,
        key?: NodeKey,
    ) {
        super(ids, key);
        this.__ids = ids;
    }

    $config () {
        return this.config("search-result", {
            extends: MarkNode,
        });
    }

    afterCloneFrom (prevNode: this): void {
        super.afterCloneFrom(prevNode);
        this.__ids = prevNode.__ids;
    }

    updateFromJSON (serializedNode: LexicalUpdateJSON<SerializedSearchResultNode>): this {
        return super.updateFromJSON(serializedNode).setIDs(serializedNode.ids);
    }

    insertNewAfter (_selection: RangeSelection, restoreSelection: boolean = true): ElementNode | null {
        const searchResultNode = $createSearchResultNode(this.__ids);
        this.insertAfter(searchResultNode, restoreSelection);
        return searchResultNode;
    }

    // Defer to MarkNode for the <mark> itself: it applies `config.theme.mark`
    // AND `config.theme.markOverlap` when __ids.length > 1, which is free
    // multi-tag styling we would lose by hand-rolling the element. We only add
    // our own class and the badge host on top.
    //
    // The badge is REAL chrome, not a pseudo-element: `setDOMUnmanaged` makes
    // Lexical's mutation-attribution up-walk terminate here
    // (LexicalMutations.ts:127), so React may render arbitrarily deep inside it
    // without the observer evicting the DOM as foreign.
    createDOM (config: EditorConfig): HTMLElement {
        const mark = super.createDOM(config);
        addClassNamesToElement(mark, "auohp-search-result");

        const badge = document.createElement("span");
        badge.className = SEARCH_RESULT_BADGE_CLASS;
        setDOMUnmanaged(badge);
        mark.prepend(badge);

        return mark;
    }

    // Tell the reconciler its managed range starts AFTER the badge. `after` is a
    // boundary node reference rather than an index (LexicalDOMSlot.ts:64), so
    // nothing needs recomputing when children churn --- and `resolveChildIndex`
    // then handles DOM-caret -> lexical-offset mapping for free.
    //
    // Re-find the badge from `element` rather than closing over the one
    // createDOM built: getDOMSlot runs against the latest node version, and
    // clone() mints new instances constantly.
    getDOMSlot (element: HTMLElement) {
        const badge = element.querySelector<HTMLElement>(`:scope > .${ SEARCH_RESULT_BADGE_CLASS }`);
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

export function $createSearchResultNode (ids: readonly string[] = NO_IDS, key?: NodeKey): SearchResultNode {
    return $applyNodeReplacement(new SearchResultNode(ids, key));
}

export function $isSearchResultNode (node?: LexicalNode): node is SearchResultNode {
    return node instanceof SearchResultNode;
}
