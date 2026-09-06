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
    static getType (): string {
        return "decorator-host";
    }

    static clone (node: DecoratorHostNode): DecoratorHostNode {
        return new DecoratorHostNode(node.__key);
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
        return false;
    }

    getDOMSlot (element: HTMLElement) {
        const badge = element.querySelector<HTMLElement>(`:scope > .${ DECORATOR_HOST_BADGE_CLASS }`);
        return badge ? super.getDOMSlot(element).withAfter(badge) : super.getDOMSlot(element);
    }
}

export function $createDecoratorHostNode (key?: NodeKey): DecoratorHostNode {
    return $applyNodeReplacement(new DecoratorHostNode(key));
}

export function $isDecoratorHostNode (node?: LexicalNode): node is DecoratorHostNode {
    return node instanceof DecoratorHostNode;
}
