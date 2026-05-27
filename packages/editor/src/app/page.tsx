"use client";

import { $getRoot, $getSelection } from "lexical";
import { useEffect } from "react";

import { AutoFocusPlugin } from "@lexical/react/LexicalAutoFocusPlugin";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";

const theme = {};

function onError(error) {
    console.error(error);
}

export default function Page() {
    const initialConfig = {
        namespace: "MyEditor",
        theme,
        onError,
    };

    return (
        <>
            <button className=" spectrum-Button spectrum-Button--fill spectrum-Button--accent spectrum-Button--sizeM " id="button-ajge1">

                <span className="spectrum-Button-label">Edit</span>


            </button>
            <LexicalComposer initialConfig={ initialConfig }>
                <RichTextPlugin
                    contentEditable={ (
                        <ContentEditable
                            aria-placeholder="Enter some text..."
                            placeholder={ <div>Enter some text...</div> } />
                    ) }
                    ErrorBoundary={ LexicalErrorBoundary } />
                <HistoryPlugin />
                <AutoFocusPlugin />
            </LexicalComposer>
        </>
    );
}
