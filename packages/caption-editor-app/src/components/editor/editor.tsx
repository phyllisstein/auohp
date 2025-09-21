"use client";

import * as R from "ramda";
import { useEffect, useMemo } from "react";
import { createEditor, type Descendant, Path } from "slate";
import { withHistory } from "slate-history";
import {
    Editable,
    Slate,
    withReact,
} from "slate-react";
import { Statement } from "./statement";

const EMPTY: Descendant[] = [{
    children: [
        {
            children: [
                {
                    children: [{ text: "" }],
                    endTime: 98.42,
                    startTime: 97.93,
                    type: "word",
                    word: "",
                },
            ],
            endTime: 104.42,
            speaker: "SPEAKER_01",
            startTime: 97.93,
            transcription: "",
            type: "segment",
            uid: "0",
        },
    ],
    type: "transcript",
}];


const Element = props => {
    const { attributes, children, element } = props;
    switch (element.type) {
        case "statement":
            return <Statement { ...props } />;
        case "word":
            return <span { ...attributes } data-testid="word">{ children }</span>;
        default:
            return <div { ...attributes } data-testid="blank-div">{ children }</div>;
    }
};

const renderElement = props => <Element { ...props } />;
const renderLeaf = ({ attributes, children }) => <span { ...attributes }>{ children }</span>;

const withCaptions = editor => {};

const withInlines = editor => {
    const { isInline } = editor;

    editor.isInline = element => {
        return element.type === "word" ? true : isInline(element);
    };

    return editor;
};

export function Editor({ editorTranscript, initialContent = EMPTY }) {
    const editor = useMemo(
        () => withInlines(withReact(withHistory(createEditor()))),
        [],
    );

    useEffect(() => {
        if (!editorTranscript || !editorTranscript.children || !editor) return;

        const children = [...editor.children];
        children.forEach(node => editor.apply({ node, path: [0], type: "remove_node" }));
        editor.apply({ node: editorTranscript, path: [0], type: "insert_node" });
    }, [editorTranscript]);

    const handleEdit = async value => {
        console.log({ operations: editor.operations });

        const changes = editor.operations.filter(
            op => "set_selection" !== op.type,
        );
        if (R.isEmpty(changes)) return;

        // TODO: Handle and batch multiple changes
        if (changes.length > 1) {
            return;
        }
        const change = changes[0];
        const path = Path.parent(change.path);
        const node = value[path[0]].children[path[1]];
        console.log({ node, value });

        const res = await fetch("/api/transcript", {
            body: JSON.stringify(node),
            headers: {
                "Content-Type": "application/json",
            },
            method: "PUT",
        });

        if (!res.ok) {
            console.error("Failed to update transcript");
        }

        const json = await res.json();
        console.log(json);
    // editor.apply({type: 'set_node', path, properties: json})
    };

    return (
        <Slate
            editor={ editor }
            initialValue={ initialContent }
            onChange={ handleEdit }>
            <Editable renderElement={ renderElement } renderLeaf={ renderLeaf } />
        </Slate>
    );
}
