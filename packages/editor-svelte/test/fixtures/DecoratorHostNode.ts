import {
    $applyNodeReplacement,
    ElementNode,
    setDOMUnmanaged,
    type EditorConfig,
    type LexicalNode,
    type NodeKey,
} from "lexical";

// A minimal ElementNode with an unmanaged badge host, shaped like TagChipNode
// but stripped of everything not needed to exercise registerSvelteDecorator:
// no MarkNode, no ids, no serialization. Test-only.
export const DECORATOR_HOST_BADGE_CLASS = "decorator-host__badge";

export class DecoratorHostNode extends ElementNode {
    // Test-only escape hatch: force updateDOM() to report "rebuild me" so a
    // caller can produce a genuine "updated" mutation whose element identity
    // actually changes, distinct from a no-op markDirty(). Not cloned --
    // deliberately per-instance-at-creation-time, since $applyNodeReplacement
    // and clone() always go through the constructor.
    __forceRebuild: boolean;

    constructor (forceRebuild: boolean = false, key?: NodeKey) {
        super(key);
        this.__forceRebuild = forceRebuild;
    }

    static getType (): string {
        return "decorator-host";
    }

    static clone (node: DecoratorHostNode): DecoratorHostNode {
        return new DecoratorHostNode(node.__forceRebuild, node.__key);
    }

    createDOM (_config: EditorConfig): HTMLElement {
        const element = document.createElement("div");

        const badge = document.createElement("span");
        badge.className = DECORATOR_HOST_BADGE_CLASS;
        setDOMUnmanaged(badge);
        element.append(badge);

        return element;
    }

    updateDOM (): boolean {
        return this.getLatest().__forceRebuild;
    }

    getDOMSlot (element: HTMLElement) {
        const badge = element.querySelector<HTMLElement>(`:scope > .${ DECORATOR_HOST_BADGE_CLASS }`);
        return badge ? super.getDOMSlot(element).withAfter(badge) : super.getDOMSlot(element);
    }
}

export function $createDecoratorHostNode (forceRebuild: boolean = false, key?: NodeKey): DecoratorHostNode {
    return $applyNodeReplacement(new DecoratorHostNode(forceRebuild, key));
}

export function $isDecoratorHostNode (node?: LexicalNode): node is DecoratorHostNode {
    return node instanceof DecoratorHostNode;
}
