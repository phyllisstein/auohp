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
import { type JSX } from "react";
import { formatTimestamp } from "@/lexical/shared";
import styled, { createGlobalStyle, css } from "styled-components";
import numberSignSVG from "./number.sign.square.svgo.svg?inline";
import { MarkNode } from "@lexical/mark";

const NO_IDS: readonly string[] = [];

// -----------------------------------------------------------------------------
// StatementNode --- the Lexical analogue of the Slate `statement` element.
//
// It extends ElementNode (a block that HOLDS the editable TextNode children) and
// carries the graph identity (uid) plus the caption window (startTime/endTime).
//
// The non-editable timestamps are REAL chrome DOM --- the direct analogue of
// Slate's `contentEditable={false}` child --- not a CSS pseudo-element. createDOM
// returns a wrapper holding (a) a chrome column and (b) an inner content element;
// getDOMSlot re-points the reconciler at the content element so it only ever
// manages the editable text there, never the chrome sibling. The chrome is also
// marked `setDOMUnmanaged` (the primitive DecoratorNode DOM uses) so selection and
// reconciliation ignore it entirely. This is what lets chrome grow past a text
// label --- speaker <Select>s, confidence meters, tag affordances --- which a
// pseudo-element (text-only, max two per element, no React) could never hold.
// -----------------------------------------------------------------------------
type SerializedStatementNode = Spread<
    {
        uid: string;
        startTime: number | null;
        endTime: number | null;
    },
    SerializedElementNode
>;

export class StatementNode extends ElementNode {
    __uid: string;
    __startTime: number | null;
    __endTime: number | null;

    static getType(): string {
        return "statement";
    }

    // `clone` is how Lexical produces the next immutable version of a node during
    // an update --- it MUST copy every custom field AND the key, or edits to one
    // version silently drop the graph identity of the next.
    static clone(node: StatementNode): StatementNode {
        return new StatementNode(node.__uid, node.__startTime, node.__endTime, node.__key);
    }

    static importJSON(serialized: SerializedStatementNode): StatementNode {
        return $createStatementNode(serialized.uid, serialized.startTime, serialized.endTime);
    }

    constructor(uid: string, startTime: number | null, endTime: number | null, key?: NodeKey) {
        super(key);
        this.__uid = uid;
        this.__startTime = startTime;
        this.__endTime = endTime;
    }

    // Getters route through `getLatest()` so reads always see the current version
    // of the node within an update, never a stale snapshot captured by closure.
    getUid(): string {
        return this.getLatest().__uid;
    }

    getStartTime(): number | null {
        return this.getLatest().__startTime;
    }

    getEndTime(): number | null {
        return this.getLatest().__endTime;
    }

    createDOM(_config: EditorConfig): HTMLElement {
        const dom = document.createElement("div");
        dom.className = "auohp-statement";
        dom.setAttribute("data-uid", this.__uid);

        // Non-editable chrome column: two stacked timestamps (start over end),
        // built as real elements so this can later hold structure/React that a
        // pseudo-element cannot. `setDOMUnmanaged` tells the reconciler this DOM
        // is not its concern; contentEditable=false keeps the caret out.
        const chrome = document.createElement("div");
        chrome.className = "auohp-statement__chrome";
        chrome.contentEditable = "false";
        chrome.append(this.#timeElement(this.__startTime), this.#timeElement(this.__endTime));
        setDOMUnmanaged(chrome);

        // The content element the reconciler DOES manage (see getDOMSlot): the
        // editable TextNode children live here, beside --- not inside --- the chrome.
        const content = document.createElement("div");
        content.className = "auohp-statement__content";

        dom.append(chrome, content);
        return dom;
    }

    // Point the reconciler's child-management slot at the inner content element
    // rather than the wrapper, so managed children never mingle with the chrome.
    // `ElementDOMSlot.resolveLeafPosition` handles DOM-caret -> lexical-offset
    // mapping for this wrap pattern, so selection needs no hand-rolled math.
    getDOMSlot(element: HTMLElement) {
        const content = element.querySelector<HTMLElement>(".auohp-statement__content") ?? element;
        return super.getDOMSlot(element).withElement(content);
    }

    // Return `false` --- Lexical keeps managing our text children --- but first
    // refresh the chrome timestamps if the caption window shifted (e.g. a split).
    updateDOM(prevNode: StatementNode, dom: HTMLElement): boolean {
        if (prevNode.__startTime !== this.__startTime || prevNode.__endTime !== this.__endTime) {
            const times = dom.querySelectorAll<HTMLElement>(
                ".auohp-statement__chrome > .auohp-statement__time",
            );
            if (times[0]) {
                times[0].textContent = formatTimestamp(this.__startTime ?? 0);
            }
            if (times[1]) {
                times[1].textContent = formatTimestamp(this.__endTime ?? 0);
            }
        }
        return false;
    }

    exportJSON(): SerializedStatementNode {
        return {
            ...super.exportJSON(),
            type: "statement",
            version: 1,
            uid: this.__uid,
            startTime: this.__startTime,
            endTime: this.__endTime,
        };
    }

    #timeElement(time: number | null): HTMLElement {
        const el = document.createElement("span");
        el.className = "auohp-statement__time";
        el.textContent = formatTimestamp(time ?? 0);
        return el;
    }
}

export function $createStatementNode(
    uid: string,
    startTime: number | null,
    endTime: number | null,
): StatementNode {
    // `$applyNodeReplacement` runs any registered node-replacement hooks and is
    // the idiomatic constructor wrapper --- cheap insurance even when we register
    // no replacements today.
    return $applyNodeReplacement(new StatementNode(uid, startTime, endTime));
}

export function $isStatementNode(node: LexicalNode | null | undefined): node is StatementNode {
    return node instanceof StatementNode;
}

// -----------------------------------------------------------------------------
// TagChipNode --- React-in-editor, the hard way.
//
// The obvious move is a DecoratorNode, whose `decorate()` returns JSX that the
// reconciler mounts at the node's own position. We can't use it. A DecoratorNode
// is a LEAF: `LexicalNode.getTextContent()` returns "" for one, and
// PersistenceExtension ships `statement.getTextContent()` to the server as the
// authoritative Statement.text. A decorator chip would silently delete the words
// it was tagging.
//
// So the chip extends MarkNode (hence ElementNode) and its children ARE the
// tagged text --- transparent to getTextContent, copy/paste, and search. That
// costs us `decorate()`, since only DecoratorNodes have one. React gets in by
// the other door instead: createDOM builds an unmanaged badge span, and
// TagChipPortals (extensions.tsx) portals <TagChip/> into it, driven by a
// mutation listener. Pull becomes push.
//
// MarkNode also brings semantics we would otherwise hand-roll: __ids with
// overlap merging, canInsertTextBefore/After() === false and canBeEmpty() ===
// false (so the chip is already sealed at its boundaries), and isInline() ===
// true.
// -----------------------------------------------------------------------------
const TagChipContainer = styled.span`
    position: absolute;
    top: 0;
    left: -1em;
    z-index: -1;

    display: block;
    width: calc(100% + 1.6em);
    height: 100%;

    color: #0B0B0B;
    font-weight: 600;
    font-size: 100%;

    background: #7DD3FC;

    user-select: none;

    &::before {
        position: absolute;
        left: 0;

        display: block;
        width: 0.8em;
        height: 0.8em;

        color: #000 !important;

        content: ${ () => `url("${ numberSignSVG }") ` };
        fill: #000 !important;
        stroke: #000 !important;
    }
`;

export const TagMarkStyles = createGlobalStyle`
    .auohp-tag-chip {
        position: relative;

        display: inline-block;
        margin: 0 1.5rem;

        background: none;
    }
`;

type SerializedTagChipNode = Spread<{ ids: string[] }, SerializedElementNode>;

// Shared by createDOM (which writes it) and getDOMSlot (which must re-find the
// element it names) --- keeping them in sync by construction rather than by
// two matching string literals.
export const BADGE_CLASS = "auohp-tag-chip__badge";

// The chip's React face. It is NOT rendered in place by Lexical --- MarkNode is
// an ElementNode, so there is no `decorate()` hook --- it is portalled into the
// unmanaged badge span that TagChipNode.createDOM builds (see TagChipPortals).
//
// It receives only a NodeKey. Everything else is read back out of EditorState
// via `editor.read()` / `editor.update()`, which keeps the component a pure
// function of editor state rather than a second copy of it.
export function TagChip({ nodeKey }: { nodeKey: NodeKey }): JSX.Element {
    return (
        <>
            <TagChipContainer className="tag-chip__container" />
        </>
    );
}

export class TagChipNode extends MarkNode {
    static clone(node: TagChipNode): TagChipNode {
        return new TagChipNode(node.__ids, node.__key);
    }

    constructor(
        ids: readonly string[] = NO_IDS,
        key?: NodeKey,
    ) {
        super(ids, key);
        this.__ids = ids;
    }

    $config() {
        return this.config("tag-chip", {
            extends: MarkNode,
        });
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

    // Defer to MarkNode for the <mark> itself: it applies `config.theme.mark`
    // AND `config.theme.markOverlap` when __ids.length > 1, which is free
    // multi-tag styling we would lose by hand-rolling the element. We only add
    // our own class and the badge host on top.
    //
    // The badge is REAL chrome, not a pseudo-element: `setDOMUnmanaged` makes
    // Lexical's mutation-attribution up-walk terminate here
    // (LexicalMutations.ts:127), so React may render arbitrarily deep inside it
    // without the observer evicting the DOM as foreign.
    createDOM(config: EditorConfig): HTMLElement {
        const mark = super.createDOM(config);
        addClassNamesToElement(mark, "auohp-tag-chip");

        const badge = document.createElement("span");
        badge.className = BADGE_CLASS;
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
    getDOMSlot(element: HTMLElement) {
        const badge = element.querySelector<HTMLElement>(`:scope > .${ BADGE_CLASS }`);
        return badge ? super.getDOMSlot(element).withAfter(badge) : super.getDOMSlot(element);
    }

    // NOTE: no updateDOM override --- MarkNode's own implementation maintains
    // the overlap class as __ids crosses 1 <-> 2, and returns false so our DOM
    // is never rebuilt.

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

export function $isTagChipNode(node?: LexicalNode) {
    return node instanceof TagChipNode;
}
