import { createFileRoute } from "@tanstack/react-router";
import { AutoFocusPlugin } from "@lexical/react/LexicalAutoFocusPlugin";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";

export const Route = createFileRoute("/")({
    component: Page,
});

const theme = {};

function onError(error: Error) {
    console.error(error);
}

function Page() {
    const initialConfig = {
        namespace: "MyEditor",
        onError,
        theme,
    };

    return (
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
    );
}
