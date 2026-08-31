import { createFileRoute } from "@tanstack/react-router";
import { useId, useMemo, useRef, type RefObject } from "react";
import { LexicalExtensionComposer } from "@lexical/react/LexicalExtensionComposer";
import { useMutation, useReadQuery } from "@apollo/client/react";
import { useSignalEffect } from "@preact/signals-react";
import { createGlobalStyle } from "styled-components";
import { playhead } from "@/playhead";
import { defineAuohpEditorExtension } from "@/lexical/extensions";
import { TRANSCRIPT_QUERY, EDIT_STATEMENT_MUTATION } from "@/queries";

// FIXME: Constructing URLs for the caption endpoint and the public video URI
// should be a server-side concern (return a Video node, return Caption metadata).
const {
    VITE_AUOHP_PUBLIC: AUOHP_PUBLIC,
    VITE_AUOHP_API_URI: AUOHP_API_URI,
} = import.meta.env;


// Drives the <video> from the shared playhead. Identical machinery to the Slate
// route --- kept here rather than in an extension because it needs the video ref.
function useVideoSync (player: RefObject<HTMLVideoElement | null>) {
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
        user-select: none;

        display: flex;
        flex-direction: column;
        flex-shrink: 0;

        min-width: 6rem;

        font-family: monospace;
        font-size: 0.75rem;
        color: #888;
    }

    .auohp-statement__content {
        flex: 1;
    }
`;


// The trailing underscore on `$interviewNumber_` is TanStack's de-nesting
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


function Page () {
    const interviewNumber = Number.parseInt(Route.useParams().interviewNumber);
    const { transcriptQueryRef } = Route.useLoaderData();
    const { data: transcriptData } = useReadQuery(transcriptQueryRef);

    const [editStatement, editStatementResult] = useMutation(EDIT_STATEMENT_MUTATION);

    const fallbackId = useId();
    const player = useRef<HTMLVideoElement>(null);
    useVideoSync(player);

    const interviewUid = transcriptData?.interview?.uid ?? fallbackId;
    const { statements } = transcriptData.interview.transcript;

    // The whole editor is now one value. Everything the old route spelled out in
    // JSX --- namespace, node registration, onError, RichTextPlugin, HistoryPlugin,
    // and the five bespoke plugins --- folds into `defineAuohpEditorExtension`.
    //
    // This also replaces the Slate route's `key` remount trick. LexicalExtensionComposer
    // memoises on the extension's identity and disposes the old editor when it
    // changes, so the memo deps are the editor's lifetime --- a fresh editor and a
    // fresh seed accompany each interview, and nothing else.
    //
    // `interview.uid` is the only dependency, and the omissions are deliberate
    // rather than sloppy --- this dep array is the editor's lifetime, so anything
    // listed here is something we are willing to destroy the user's document over.
    //
    // `statements` is an Apollo cache object with a fresh identity after every
    // write-back, so listing it would tear down the editor on each autosave. Any
    // per-render Apollo result (a `useLazyQuery` state tuple, say) is worse still:
    // it changes identity on every state transition, so the editor would be rebuilt
    // mid-search. Live query data therefore does not travel through here at all ---
    // it flows through extension signals, which are writable after construction.
    // See SearchInterviewExtension.
    //
    // `editStatement` is captured deliberately for the same reason: PersistenceExtension
    // reads it back out of a signal at fire time rather than closing over it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    const extension = useMemo(() =>
        defineAuohpEditorExtension({
            editStatement,
            statements,
        }), [interviewUid]);

    const statementHash = editStatementResult.data?.editStatement?.newHash;

    return (
        <>
            {
                transcriptData?.interview?.interviewee
                    ? <title>{ transcriptData.interview.interviewee } | AUOHP Editor</title>
                    : <title>AUOHP Editor</title>
            }
            <div>
                <EditorStyle />
                <video
                    ref={ player }
                    controls
                    crossOrigin="anonymous"
                    onTimeUpdate={ ev => playhead.timestamp.value = (ev.target as HTMLVideoElement).currentTime }>
                    <source src={ `${ AUOHP_PUBLIC }/videos/${ interviewNumber }.mp4` } type="video/mp4" />
                    {
                        interviewUid && <track default key={ statementHash } kind="captions" src={ `${ AUOHP_API_URI }/interview/${ interviewNumber }/vtt` } srcLang="en" label="English" />
                    }
                </video>

                <LexicalExtensionComposer extension={ extension } />
            </div>
        </>
    );
}
