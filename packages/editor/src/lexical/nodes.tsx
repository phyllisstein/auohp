import {
    $applyNodeReplacement,
    $create,
    $createParagraphNode,
    $createTextNode,
    $setSlot,
    DecoratorNode,
    ElementNode,
    setDOMUnmanaged,
    type EditorConfig,
    type LexicalEditor,
    type LexicalNode,
    type LexicalUpdateJSON,
    type NodeKey,
    type SerializedElementNode,
    type SlotChildNode,
    type Spread,
    type DOMExportOutput,
    $getDocument,
    type RangeSelection,
} from "lexical";
import { type JSX } from "react";
import { formatTimestamp } from "@/lexical/shared";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { useLexicalSlotRef } from "@lexical/react/useLexicalSlotRef";
import styled from "styled-components";
import numberSignSVG from "./number.sign.square.svgo.svg?inline";
import { MarkNode } from "@lexical/mark";
import { rest } from "es-toolkit";


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
        chrome.append(
            this.#timeElement(this.__startTime),
            this.#timeElement(this.__endTime),
        );
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
            const times = dom.querySelectorAll<HTMLElement>(".auohp-statement__chrome > .auohp-statement__time");
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


export function $createStatementNode(uid: string, startTime: number | null, endTime: number | null): StatementNode {
    // `$applyNodeReplacement` runs any registered node-replacement hooks and is
    // the idiomatic constructor wrapper --- cheap insurance even when we register
    // no replacements today.
    return $applyNodeReplacement(new StatementNode(uid, startTime, endTime));
}


export function $isStatementNode(node: LexicalNode | null | undefined): node is StatementNode {
    return node instanceof StatementNode;
}


// -----------------------------------------------------------------------------
// TagChipNode --- React-in-editor.
//
// A DecoratorNode: a leaf whose visual body is a REACT component rendered by
// @lexical/react's decorator machinery. This is the primitive future graph-tagging
// needs --- an inline node that lives in EditorState (serialises, undoes,
// participates in selection) yet paints itself with arbitrary React. `decorate()`
// is the state -> React bridge: whatever it returns is mounted into the editor's
// DOM at this node's position and re-rendered when the node changes.
// -----------------------------------------------------------------------------
const TagChipContainer = styled.div`
    display: inline-block;
    margin: 0.4em;
    padding: 0.4em;

    color: #0B0B0B;
    font-weight: 600;
    font-size: 85%;

    background: #7DD3FC;
    border-radius: 11em;

    user-select: none;

    positiopn: relative;

    & p {
        margin: 0 0 0 1.5em;
        border: 0;
    }

    & svg path,
    & svg rect {
        fill: #000 !important;
        stroke: #000 !important;
    }

    &::before {
        position: absolute;

        display: block;
        align-items: center;
        justify-content: center;
        width: 1em;
        height: 1em;

        color: #000 !important;

        content: ${ () => `url("${ numberSignSVG }") ` };
        fill: #000 !important;
        stroke: #000 !important;
    }
`;


type SerializedTagChipNode = Spread<{ ids: string[] }, SerializedElementNode>;


function TagChip({ nodeKey }: { nodeKey: NodeKey }) {
    const [editor] = useLexicalComposerContext();
    const tagRef = useLexicalSlotRef<HTMLDivElement>(editor, nodeKey, "tag");

    console.log(tagRef);

    return (
        <>
            <TagChipContainer
                ref={ tagRef }
                className="tag-chip-container">
            </TagChipContainer>
        </>
    );
}


export class TagChipNode extends MarkNode {
    static getType(): string {
        return "tag-chip";
    }

    static clone(node: TagChipNode): TagChipNode {
        return new TagChipNode(node.__ids, node.__key);
    }

    constructor(ids: readonly string[] = NO_IDS, key?: NodeKey) {
        super(ids, key);
        this.__ids = ids;
    }

    $config() {
        return this.config("tag-chip", {
            extends: MarkNode,
            slots: ["tag"],
        });
    }

    afterCloneFrom(prevNode: this): void {
        super.afterCloneFrom(prevNode);
        this.__ids = prevNode.__ids;
    }

    updateFromJSON(serializedNode: LexicalUpdateJSON<SerializedTagChipNode>): this {
        return super.updateFromJSON(serializedNode).setIDs(serializedNode.ids);
    }


    insertNewAfter(
        selection: RangeSelection,
        restoreSelection: boolean = true,
    ): ElementNode | null {
        const tagChipNode = $createTagChipNode(this.__ids);
        this.insertAfter(tagChipNode, restoreSelection);
        return tagChipNode;
    };

    // Inline so the chip flows within a statement's text rather than breaking it
    // onto its own line.
    // isInline(): true {
    //     return true;
    // }

    // isShadowRoot(): true {
    //     return true;
    // }

    createDOM(config: EditorConfig): HTMLElement {
        const mark = super.createDOM(config);
        return mark;
    }

    // The DOM host never changes --- React owns everything inside it --- so the
    // reconciler can skip this node entirely.
    // updateDOM(): boolean {
    //     return false;
    // }

    exportJSON(): SerializedTagChipNode {
        const parent = super.exportJSON();
        console.log(parent);

        const serial = {
            ...parent,
            type: "tag-chip",
            version: 1,
        };

        console.log({ serial });

        return serial;
    }

    // Export as a fragment since it "is" the slot wrapper created by the host
    exportDOM(editor: LexicalEditor): DOMExportOutput {
        return { element: document.createDocumentFragment() };
    }

    decorate(): JSX.Element {
        return <TagChip nodeKey={ this.__key } />;
    }

    collapseAtStart(): true {
        return true;
    }

    getIDs(): string[] {
        return [...this.getLatest().__ids];
    }
}


export function $createTagChipNode(
    ids: readonly string[] = NO_IDS,
): TagChipNode {
    return $applyNodeReplacement(new TagChipNode(ids));
}

export function $isTagChipNode(node?: LexicalNode) {
    return node instanceof TagChipNode;
}


export class EmptyEditorNode extends DecoratorNode<JSX.Element> {
    __label: string;
}

// See: https://github.com/facebook/lexical/blob/main/packages/lexical-playground/src/nodes/SlotContainerNode/index.tsx
export class SlotContainerNode extends ElementNode {
    $config() {
        return this.config("slot-container", { extends: ElementNode });
    }

    createDOM(): HTMLElement {
        const div = document.createElement("div");
        div.className = "auohp-slot-container";
        return div;
    }

    // Export as a fragment since it "is" the slot wrapper created by the host
    exportDOM(editor: LexicalEditor): DOMExportOutput {
        return { element: document.createDocumentFragment() };
    }

    updateDOM(): false {
        return false;
    }

    isShadowRoot(): boolean {
        return true;
    }

    collapseAtStart(): true {
        return true;
    }
}
