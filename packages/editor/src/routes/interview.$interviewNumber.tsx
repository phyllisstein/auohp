import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { type BaseEditor, createEditor, type Descendant, Editor, Element, Node, Text, Transforms } from "slate";
import { Slate, Editable, type ReactEditor, withReact } from "slate-react";
import { Badge } from "@react-spectrum/s2/Badge";
import { gql } from "@apollo/client";
import { useQuery } from "@apollo/client/react";


const TRANSCRIPT_QUERY = gql`
    query TranscriptQuery($interviewNumber: Int!) {
        health
        interviewTranscript(number: $interviewNumber) {
            uid
            statements {
                uid
                startTime
                endTime
                text
            }
        }
    }
`;


type BaseText = { text: string };

interface WordElement {
    type: "word";
    startTime: number;
    endTime: number;
    children: BaseText[];
}

interface StatementElement {
    type: "statement";
    uid: string;
    startTime: number;
    endTime: number;
    children: Array<WordElement | BaseText>;
}

declare module "slate" {
    interface CustomTypes {
        Editor: BaseEditor & ReactEditor;
        Element: StatementElement | WordElement;
        Text: BaseText;
    }
}


export const Route = createFileRoute("/interview/$interviewNumber")({
    component: Page,
});


const formatTimestamp = (timestamp: number) =>
    Temporal.Duration.from({ seconds: Math.round(timestamp) })
        .round({
            largestUnit: "hours",
            smallestUnit: "seconds",
        })
        .toLocaleString("en-US", {
            style: "digital",
            hoursDisplay: "auto",
            hours: "numeric",
        });


const initialValue: Descendant[] = [
    {
        type: "statement",
        startTime: 0,
        endTime: 100,
        uid: "statement-1",
        children: [
            { text: "ACT UP Oral History Project" },
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


const StatementElement = ({ attributes, children, element }) => {
    return (
        <div key={ element.uid } { ...attributes }>
            <div contentEditable={ false } style={{ userSelect: "none" }}>
                { formatTimestamp(element.startTime) }
                { " " }
                -
                { formatTimestamp(element.endTime) }
            </div>
            { children }
        </div>
    );
};




>>>>>>> 3a823cb (fixup! Fetch Interview by route param)
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
    const [editor] = useState<Editor>(() => withReact(createEditor()));
    // Slate does not normalize initialValue on its own --- it trusts the tree is
    // already valid. Our initialValue has adjacent inline words (an invalid shape),
    // so we run one forced pass here to let `normalizeNode` intersperse the spaces.
    const [value] = useState<Descendant[]>(() => {
        editor.children = initialValue;
        Editor.normalize(editor, { force: true });
        return editor.children;
    });

    const interviewNumber = Number.parseInt(Route.useParams().interviewNumber);
    const { data, loading, error } = useQuery(TRANSCRIPT_QUERY, { variables: { interviewNumber } });
    const variant = loading ? "neutral" : error ? "negative" : "positive";

    return (
        <>
            <Badge variant={ variant } size="L">
                { loading ? "Loading..." : error ? "Error" : data.health }
            </Badge>
            <Slate editor={ editor } initialValue={ initialValue }>
                <Editable renderElement={ renderElement } />
            </Slate>
        </>
    );
}
