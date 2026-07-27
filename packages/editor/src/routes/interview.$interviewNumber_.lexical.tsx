import { createFileRoute } from "@tanstack/react-router";
import { useRef, type JSX, type RefObject } from "react";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { useMutation, useReadQuery } from "@apollo/client/react";
import { useSignalEffect } from "@preact/signals-react";
import { createGlobalStyle } from "styled-components";
import { playhead } from "@/playhead";
import { StatementNode, TagChipNode } from "@/lexical/nodes";
import { INSERT_TAG_CHIP_COMMAND } from "@/lexical/commands";
import {
    LatencyMeter,
    PersistencePlugin,
    SeedPlugin,
    SplitStatementPlugin,
    StatementSeekPlugin,
    TagChipPlugin,
} from "@/lexical/plugins";
// Reuse (do NOT duplicate) the incumbent Slate route's GraphQL operations and
// loader shape so the two editors talk to the backend identically. The only
// honest way to compare Slate vs Lexical is to hold everything else constant.
import { EDIT_STATEMENT_MUTATION, TRANSCRIPT_QUERY } from "./interview.$interviewNumber";


// FIXME: Constructing URLs for the caption endpoint and the public video URI
// should be a server-side concern (return a Video node, return Caption metadata).
const {
    VITE_AUOHP_PUBLIC: AUOHP_PUBLIC,
    VITE_AUOHP_API_URI: AUOHP_API_URI,
} = import.meta.env;


// Drives the <video> from the shared playhead. Identical machinery to the Slate
// route --- kept here rather than in a plugin because it needs the video ref.
function useVideoSync(player: RefObject<HTMLVideoElement | null>) {
    useSignalEffect(() => {
        !!player.current && (player.current.currentTime = playhead.seek.value);
    });
}


// Styles for the statement wrapper, its non-editable chrome column, and the
// editable content element (see StatementNode.createDOM / getDOMSlot).
const EditorStyle = createGlobalStyle`
    .auohp-statement {
        display: flex;
        gap: 0.75rem;
        align-items: flex-start;
        padding: 0.25rem 0;
    }

    .auohp-statement__chrome {
        display: flex;
        flex-direction: column;
        flex-shrink: 0;
        min-width: 6rem;
        font-family: monospace;
        font-size: 0.75rem;
        color: #888;
        user-select: none;
    }

    .auohp-statement__content {
        flex: 1;
    }
`;


// The trailing underscore on `$interviewNumber_` is TanStack's DE-NESTING
// convention: the URL stays `/interview/:n/lexical`, but the route opts OUT of
// rendering inside the Slate route (`interview.$interviewNumber.tsx`), which has
// no <Outlet/>. So this is a sibling under root, not a child --- the Slate route
// stays untouched. The generator maintains the `_` in this path string for us.
export const Route = createFileRoute("/interview/$interviewNumber_/lexical")({
    component: Page,
    // FIXME: Handle errors gracefully
    loader: ({ context: { preloadQuery }, params }) => {
        const interviewNumber = Number.parseInt(params.interviewNumber);
        if (Number.isNaN(interviewNumber)) {
            throw new Error("Invalid interview number");
        }

        const transcriptQueryRef = preloadQuery(TRANSCRIPT_QUERY, {
            variables: {
                interviewNumber,
            },
        });

        return { transcriptQueryRef };
    },
});


function Page() {
    const interviewNumber = Number.parseInt(Route.useParams().interviewNumber);
    const { transcriptQueryRef } = Route.useLoaderData();
    const { data: transcriptData } = useReadQuery(transcriptQueryRef);

    const [editStatement] = useMutation(EDIT_STATEMENT_MUTATION);

    const player = useRef<HTMLVideoElement>(null);
    useVideoSync(player);

    const { statements, interview } = transcriptData.interviewTranscript;

    const initialConfig = {
        namespace: "auohp-lexical-spike",
        // Registering our custom nodes is how Lexical learns to construct,
        // serialise, and reconcile them. Forget one and the editor throws on the
        // first attempt to create it --- a loud, discoverable failure mode.
        nodes: [StatementNode, TagChipNode],
        onError: (error: Error) => {
            throw error;
        },
        // We seed via SeedPlugin (tagged "seed") rather than initialConfig, so the
        // persistence/latency listeners can tell the seed apart from real edits.
        theme: {},
    };

    return (
        <div>
            <EditorStyle />
            <video
                ref={ player }
                controls
                crossOrigin="anonymous"
                onTimeUpdate={ ev => playhead.timestamp.value = (ev.target as HTMLVideoElement).currentTime }>
                <source src={ `${ AUOHP_PUBLIC }/videos/${ interviewNumber }.mp4` } type="video/mp4" />
                {
                    interview.uid && <track kind="captions" src={ `${ AUOHP_API_URI }/interview/${ interview.uid }/vtt` } srcLang="en" label="English" />
                }
            </video>

            {/* `key` remounts the whole composer (fresh editor + fresh seed) when
                the interview changes --- mirrors the Slate route's `key`. */}
            <LexicalComposer key={ interview.uid } initialConfig={ initialConfig }>
                <div style={{ display: "flex", gap: "1rem", alignItems: "center", padding: "0.5rem 0" }}>
                    <TagButton />
                    <LatencyMeter />
                </div>

                <RichTextPlugin
                    contentEditable={ <ContentEditable /> }
                    ErrorBoundary={ LexicalErrorBoundary } />
                <HistoryPlugin />
                <SeedPlugin statements={ statements } />
                <StatementSeekPlugin />
                <SplitStatementPlugin />
                <TagChipPlugin />
                <PersistencePlugin editStatement={ editStatement } />
            </LexicalComposer>
        </div>
    );
}


// A trivial toolbar affordance that dispatches the typed insert command ---
// demonstrating an out-of-editor React control mutating EditorState, which then
// renders back through a React DecoratorNode.
function TagButton(): JSX.Element {
    const [editor] = useLexicalComposerContext();

    return (
        <button
            type="button"
            onClick={ () => editor.dispatchCommand(INSERT_TAG_CHIP_COMMAND, "person") }>
            Insert #person chip
        </button>
    );
}
