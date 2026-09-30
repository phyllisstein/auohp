import {
    $applyNodeReplacement,
    addClassNamesToElement,
    createGlobalStyle,
    setDOMUnmanaged,
    type EditorConfig,
    type LexicalNode,
    type LexicalUpdateJSON,
    type NodeKey,
    type SerializedElementNode,
    type Spread,
} from "lexical";
import { useEffect, useRef, type JSX } from "react";
import styled from "styled-components";
import { MarkNode } from "@lexical/mark";

const NO_IDS: readonly string[] = [];

// The badge host's face. It stays a portal target (so a per-match affordance ---
// hit counter, jump-to-next, hover popover --- has somewhere to live) but it no
// longer paints the highlight itself.
const SearchResultContainer = styled.span`
    user-select: none;

    /* Out of flow without being positioned: the badge must not consume layout
       space between the words it sits inside. It is a hook for portalled
       chrome, so it has no size of its own until something is portalled in. */
    display: inline;

    font-size: 100%;
    font-weight: 600;
    color: #0B0B0B;
`;

export const SearchResultStyles = createGlobalStyle`
    /* The highlight is now painted by the <mark> itself, inline, so it flows
       with the text and breaks correctly across lines --- which is exactly what
       a background on an inline box does for free, and what no absolutely
       positioned overlay can do.

       'box-decoration-break: clone' is the piece that is easy to miss: without
       it, a highlight spanning a line break gets its padding and rounding only
       at the outer two ends, so the fragment reads as one stretched box rather
       than two. 'clone' re-applies the decoration to each line box. */
    .auohp-search-result {
        position: relative;

        margin: 0;
        padding: 0.05em 0.15em;
        border-radius: 0.2em;

        color: inherit;

        background: #FCE94F;
        box-decoration-break: clone;
    }

    /* MarkNode applies theme.markOverlap when __ids.length > 1. Every mark here
       carries exactly one uid (its statement's), so overlap only arises if a
       future caller merges ids --- worth styling now so it degrades visibly
       rather than silently. */
    .auohp-search-result.auohp-search-result--overlap {
        background: #FCAF3E;
    }
`;

type SerializedSearchResultNode = Spread<{ ids: string[] }, SerializedElementNode>;

// Shared by createDOM (which writes it) and getDOMSlot (which must re-find the
// element it names) --- keeping them in sync by construction rather than by
// two matching string literals.
export const SEARCH_RESULT_BADGE_CLASS = "auohp-search-result__badge";

// The result's React face. It is not rendered in place by Lexical --- MarkNode is
// an ElementNode, so there is no `decorate()` hook --- it is portalled into the
// unmanaged badge span that SearchResultNode.createDOM builds.
//
// It receives only a NodeKey and a focused flag. Everything else is read back out
// of EditorState via `editor.read()` / `editor.update()`, which keeps the component
// a pure function of editor state rather than a second copy of it.
export function SearchResult ({ nodeKey, focused }: { nodeKey: NodeKey; focused: boolean }): JSX.Element {
    const container = useRef<HTMLSpanElement>(null);

    useEffect(() => {
        if (focused && container.current) {
            container.current.scrollIntoView({ behavior: "smooth", block: "center" });
        }
    }, [focused]);

    return (
        <SearchResultContainer ref={ container } data-node-key={ nodeKey } className="auohp-search-result__container" />
    );
}

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

    insertNewAfter (selection: any, restoreSelection: boolean = true): any {
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
