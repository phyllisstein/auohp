import { createFileRoute } from "@tanstack/react-router";
import { useMemo, useRef, type RefObject } from "react";
import { LexicalExtensionComposer } from "@lexical/react/LexicalExtensionComposer";
import { useMutation, useReadQuery } from "@apollo/client/react";
import { useSignalEffect } from "@preact/signals-react";
import { createGlobalStyle } from "styled-components";
import { playhead } from "@/playhead";
import { defineAuohpEditorExtension } from "@/lexical/extensions";
import { graphql } from "@/gql";


// FIXME: Constructing URLs for the caption endpoint and the public video URI
// should be a server-side concern (return a Video node, return Caption metadata).
const {
    VITE_AUOHP_PUBLIC: AUOHP_PUBLIC,
    VITE_AUOHP_API_URI: AUOHP_API_URI,
} = import.meta.env;


export const EDIT_STATEMENT_MUTATION = graphql(`
    mutation EditStatement($uid: String!, $text: String!) {
        editStatement(input: { uid: $uid, text: $text }) {
            uid
            oldHash
            newHash
            wroteEmbedding
        }
    }
`);

export const TRANSCRIPT_QUERY = graphql(`
    query Transcript($interviewNumber: Int!) {
        health
        interviewTranscript(number: $interviewNumber) {
            uid
            interview {
                uid
            }
            statements {
                uid
                startTime
                endTime
                text
            }
        }
    }
`);


// Drives the <video> from the shared playhead. Identical machinery to the Slate
// route --- kept here rather than in an extension because it needs the video ref.
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

        color: #888;
        font-size: 0.75rem;
        font-family: monospace;

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
export const Route = createFileRoute("/interview/$interviewNumber")({
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

    // The whole editor is now ONE value. Everything the old route spelled out in
    // JSX --- namespace, node registration, onError, RichTextPlugin, HistoryPlugin,
    // and the five bespoke plugins --- folds into `defineAuohpEditorExtension`.
    //
    // This also replaces the Slate route's `key` remount trick. LexicalExtensionComposer
    // memoises on the extension's IDENTITY and disposes the old editor when it
    // changes, so the memo deps ARE the editor's lifetime --- a fresh editor and a
    // fresh seed accompany each interview, and nothing else.
    //
    // Depending on `interview.uid` rather than on `statements` is load-bearing:
    // `statements` is an Apollo cache object that gets a new identity every time a
    // save writes back, so keying off it would tear down the user's editor on every
    // autosave. `editStatement` is likewise captured deliberately --- see
    // PersistenceExtension, which reads it back out of a signal at fire time.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    const extension = useMemo(() => defineAuohpEditorExtension({ editStatement, statements }), [interview.uid]);

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

            <LexicalExtensionComposer extension={ extension } />
        </div>
    );
}
