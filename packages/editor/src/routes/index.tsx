import { createFileRoute } from "@tanstack/react-router";
import { AutoFocusPlugin } from "@lexical/react/LexicalAutoFocusPlugin";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";

export const Route = createFileRoute("/")({
    component: Page,
    ssr: false,
    beforeLoad: () => {
        import("@spectrum-web-components/theme/sp-theme.js");
        import("@spectrum-web-components/theme/src/themes.js");
        import("@spectrum-web-components/theme/theme-light.js");
        import("@spectrum-web-components/theme/scale-large.js");
        import("@spectrum-web-components/button/sp-button.js");
        import("@spectrum-web-components/badge/sp-badge.js");
    },
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
        <>
            <sp-button onClick={ () => console.log("Button clicked") }>Try me</sp-button>
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
