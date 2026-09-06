import {
    $applyNodeReplacement,
    ElementNode,
    setDOMUnmanaged,
    type EditorConfig,
    type LexicalNode,
    type NodeKey,
    type RangeSelection,
    type SerializedElementNode,
    type Spread,
} from "lexical";
import { $getExtensionDependency } from "@lexical/extension";
import { formatTimestamp } from "./timestamps";
import { SYNTHETIC_UID_MARKER } from "../persistence/synthetic-uid";
import { StatementExtension } from "./StatementExtension";

// -----------------------------------------------------------------------------
// StatementNode --- the Lexical analogue of the Slate `statement` element.
//
// It extends ElementNode (a block that holds the editable TextNode children) and
// carries the graph identity (uid) plus the caption window (startTime/endTime).
//
// The non-editable timestamps are real chrome DOM, not a CSS pseudo-element.
// createDOM returns a wrapper holding (a) a chrome column and (b) an inner
// content element; getDOMSlot re-points the reconciler at the content element so
// it only ever manages the editable text there, never the chrome sibling. The
// chrome is also marked `setDOMUnmanaged` so selection and reconciliation ignore
// it entirely.
// -----------------------------------------------------------------------------
type SerializedStatementNode = Spread<
    {
        uid: string;
        startTime: number;
        endTime: number;
    },
    SerializedElementNode
>;

// Exported for the statement-seek extension (step 5), which delegates a single
// click listener from the editor root and needs these to identify the chrome
// and walk back to the statement wrapper carrying `data-uid`.
export const STATEMENT_NODE_CLASS = "auohp-statement";
export const STATEMENT_CHROME_CLASS = "auohp-statement__chrome";
const STATEMENT_CONTENT_CLASS = "auohp-statement__content";
const STATEMENT_TIME_CLASS = "auohp-statement__time";

export class StatementNode extends ElementNode {
    __uid: string;
    __startTime: number;
    __endTime: number;

    static getType (): string {
        return "statement";
    }

    // `clone` is how Lexical produces the next immutable version of a node during
    // an update --- it must copy every custom field and the key, or edits to one
    // version silently drop the graph identity of the next.
    static clone (node: StatementNode): StatementNode {
        return new StatementNode(node.__uid, node.__startTime, node.__endTime, node.__key);
    }

    static importJSON (serialized: SerializedStatementNode): StatementNode {
        return $createStatementNode(serialized.uid, serialized.startTime, serialized.endTime);
    }

    constructor (uid: string, startTime: number | null, endTime: number | null, key?: NodeKey) {
        super(key);
        this.__uid = uid;
        this.__startTime = startTime ?? 0;
        this.__endTime = endTime ?? 0;
    }

    // Getters route through `getLatest()` so reads always see the current version
    // of the node within an update, never a stale snapshot captured by closure.
    getUid (): string {
        return this.getLatest().__uid;
    }

    getStartTime (): number {
        return this.getLatest().__startTime;
    }

    getEndTime (): number {
        return this.getLatest().__endTime;
    }

    setStartTime (startTime: number | null): this {
        const writable = this.getWritable();
        writable.__startTime = startTime ?? 0;
        return writable;
    }

    setEndTime (endTime: number | null): this {
        const writable = this.getWritable();
        writable.__endTime = endTime ?? 0;
        return writable;
    }

    setUid (uid: string): this {
        const writable = this.getWritable();
        writable.__uid = uid;
        return writable;
    }

    // Called by RangeSelection.insertParagraph() when Enter splits this block.
    // Lexical has already split the TextNodes and knows which children belong to
    // the tail --- it only needs us to say what kind of node the continuation is.
    // It then moves those children into whatever we return, so inline structure
    // (tag chips and their __ids) crosses the split intact.
    //
    // Return null to refuse the split (what CodeNode does).
    insertNewAfter (selection: RangeSelection, restoreSelection = true): ElementNode | null {
        const newUid = `${ this.getUid() }${ SYNTHETIC_UID_MARKER }${ Date.now() }`;
        // Lexical constructs and calls nodes itself, so there is no constructor
        // call site to inject a per-instance Playhead through. This resolves
        // the current editor's registered StatementExtension instead --
        // per-editor by construction, and throws if the extension is missing
        // rather than silently defaulting (see PLAN.md 3.1 and its risk 4).
        const currentTime = $getExtensionDependency(StatementExtension).output.timestamp;
        const continuation = $createStatementNode(newUid, currentTime, this.getEndTime());
        this.setEndTime(currentTime);

        this.insertAfter(continuation, restoreSelection);

        return continuation;
    }

    createDOM (_config: EditorConfig): HTMLElement {
        const dom = document.createElement("div");
        dom.className = STATEMENT_NODE_CLASS;
        dom.setAttribute("data-uid", this.__uid);

        // Non-editable chrome column: two stacked timestamps (start over end),
        // built as real elements so this can later hold structure beyond a text
        // label. `setDOMUnmanaged` tells the reconciler this DOM is not its
        // concern; contentEditable=false keeps the caret out.
        const chrome = document.createElement("div");
        chrome.className = STATEMENT_CHROME_CLASS;
        chrome.contentEditable = "false";
        chrome.append(this.#timeElement(this.__startTime), this.#timeElement(this.__endTime));
        setDOMUnmanaged(chrome);

        // The content element the reconciler DOES manage (see getDOMSlot): the
        // editable TextNode children live here, beside --- not inside --- the chrome.
        const content = document.createElement("div");
        content.className = STATEMENT_CONTENT_CLASS;

        dom.append(chrome, content);
        return dom;
    }

    // Point the reconciler's child-management slot at the inner content element
    // rather than the wrapper, so managed children never mingle with the chrome.
    getDOMSlot (element: HTMLElement) {
        const content = element.querySelector<HTMLElement>(`.${ STATEMENT_CONTENT_CLASS }`) ?? element;
        return super.getDOMSlot(element).withElement(content);
    }

    // Return `false` --- Lexical keeps managing our text children --- but first
    // refresh the chrome timestamps if the caption window shifted (e.g. a split).
    updateDOM (prevNode: StatementNode, dom: HTMLElement): boolean {
        // The uid changes exactly once in a node's life: when the server answers
        // `createStatement` and the synthetic uid gives way to the real one. That
        // used to ride in on a full node replacement, which rebuilt this DOM (and
        // called `createDOM` again) as a side effect; now that the adoption is an
        // in-place field update, the attribute has to be synced here or the DOM
        // keeps advertising a uid the backend has never heard of.
        if (prevNode.__uid !== this.__uid) {
            dom.setAttribute("data-uid", this.__uid);
        }

        if (prevNode.__startTime !== this.__startTime || prevNode.__endTime !== this.__endTime) {
            const times = dom.querySelectorAll<HTMLElement>(
                `.${ STATEMENT_TIME_CLASS }`,
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

    exportJSON (): SerializedStatementNode {
        return {
            ...super.exportJSON(),
            type: "statement",
            version: 1,
            uid: this.__uid,
            startTime: this.__startTime,
            endTime: this.__endTime,
        };
    }

    #timeElement (time: number): HTMLElement {
        const el = document.createElement("span");
        el.className = STATEMENT_TIME_CLASS;
        el.textContent = formatTimestamp(time ?? 0);
        return el;
    }
}

export function $createStatementNode (
    uid: string,
    startTime: number | null,
    endTime: number | null,
): StatementNode {
    // `$applyNodeReplacement` runs any registered node-replacement hooks and is
    // the idiomatic constructor wrapper --- cheap insurance even when we register
    // no replacements today.
    return $applyNodeReplacement(new StatementNode(uid, startTime, endTime));
}

// Adopts the server's identity and timings onto an existing statement, in place.
//
// This deliberately does NOT go through `clone` + `replace`, which is the shape
// it originally had. `clone` copies `__key` (see above), so the "replacement" is
// not an independent node at all: the first `getWritable()` inside `setUid`
// resolves that key against the active EditorState and hands back the canonical
// node from the tree. The clone is discarded, the setters mutate the original,
// and `node.replace(replacement)` then replaces the node with itself --- pure
// churn that also re-parents every child, which is precisely how tag chips get
// destroyed (see `insertNewAfter`).
//
// Replacement is for changing a node's type. Changing its fields is what the
// setters are for, and they already handle versioning correctly.
export function $adoptStatementIdentity (
    node: StatementNode,
    { uid, startTime, endTime }: { uid: string; startTime: number | null; endTime: number | null },
): StatementNode {
    return node.setUid(uid).setStartTime(startTime).setEndTime(endTime);
}

export function $isStatementNode (node: LexicalNode): node is StatementNode {
    return node instanceof StatementNode;
}
