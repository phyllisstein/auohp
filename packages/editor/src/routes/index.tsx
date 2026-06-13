import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { type BaseEditor, createEditor, type Descendant, Editor, Element, Node, Text, Transforms } from "slate";
import { Slate, Editable, type ReactEditor, withReact } from "slate-react";
import { Button } from "@react-spectrum/s2/Button";
import { createLink } from "@tanstack/react-router";

type BaseText = { text: string };

interface WordElement {
    type: "word";
    startTime: number;
    endTime: number;
    children: BaseText[];
}

interface StatementElement {
    type: "statement";
    children: Array<WordElement | BaseText>;
}

declare module "slate" {
    interface CustomTypes {
        Editor: BaseEditor & ReactEditor;
        Element: StatementElement | WordElement;
        Text: BaseText;
    }
}


const ButtonLink = createLink(Button);


export const Route = createFileRoute("/")({
    component: Page,
});


const initialValue: Descendant[] = [
    {
        type: "statement",
        children: [
            { type: "word", children: [{ text: "ACT" }], startTime: 0, endTime: 1000 },
            { type: "word", children: [{ text: "UP" }], startTime: 1000, endTime: 2000 },
            { type: "word", children: [{ text: "Oral" }], startTime: 2000, endTime: 3000 },
            { type: "word", children: [{ text: "History" }], startTime: 3000, endTime: 4000 },
            { type: "word", children: [{ text: "Project" }], startTime: 4000, endTime: 5000 },
        ],
    },
];

const WordElement = ({ attributes, children }) => {
    return (
        <span style={{ display: "inline-block", marginRight: "1ex" }} { ...attributes }>
            { children }
        </span>
    );
};
const DefaultElement = props => {
    return <div { ...props.attributes }>{ props.children }</div>;
};

const renderElement = props => {
    switch (props.element.type) {
        case "word":
            return <WordElement { ...props } />;
        default:
            return <DefaultElement { ...props } />;
    }
};

const withWords = editor => {
    const { isInline, normalizeNode } = editor;

    editor.isInline = element => {
        return element.type === "word" ? true : isInline(element);
    };

    // WARNING: This step ensures `word` nodes are separated by a single space
    // in the Slate schema. Note that this materializes the "space" as an actual
    // text node in the editor tree. These nodes will have to be filtered out at
    // serialization time.
    editor.normalizeNode = ([node, path]) => {
        if (Element.isElement(node) && node.type === "statement") {
            const children = Array.from(Node.children(editor, path));
            for (let i = 0; i < children.length - 1; i++) {
                const [curr] = children[i];
                const [next, nextPath] = children[i + 1];
                // Two adjacent word elements --- insert a space text node between them.
                if (Element.isElement(curr) && curr.type === "word"
                  && Element.isElement(next) && next.type === "word") {
                    Transforms.insertNodes(editor, { text: " " }, { at: nextPath });
                    return;
                }
                // Slate inserted an empty gap node; upgrade it to a space.
                if (Text.isText(curr) && curr.text === "") {
                    const [, currPath] = children[i];
                    Transforms.insertText(editor, " ", { at: { path: currPath, offset: 0 } });
                    return;
                }
            }
        }

        normalizeNode([node, path]);
    };

    return editor;
};

function Page() {
    const [editor] = useState<Editor>(() => withWords(withReact(createEditor())));
    // Slate does not normalize initialValue on its own --- it trusts the tree is
    // already valid. Our initialValue has adjacent inline words (an invalid shape),
    // so we run one forced pass here to let `normalizeNode` intersperse the spaces.
    const [value] = useState<Descendant[]>(() => {
        editor.children = initialValue;
        Editor.normalize(editor, { force: true });
        return editor.children;
    });

    return (
        <>
            <Slate editor={ editor } initialValue={ value }>
                <Editable renderElement={ renderElement } />
            </Slate>
            <ButtonLink to="/oops" variant="accent">Save</ButtonLink>
        </>
    );
}
