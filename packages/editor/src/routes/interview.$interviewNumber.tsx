import { ClientOnly, createFileRoute } from "@tanstack/react-router";
import { useMemo, useRef, type RefObject } from "react";
import { LexicalExtensionComposer } from "@lexical/react/LexicalExtensionComposer";
import { useMutation, useReadQuery } from "@apollo/client/react";
import { useSignalEffect } from "@preact/signals-react";
import styled, { createGlobalStyle } from "styled-components";
import { playhead } from "~/playhead";
import { defineAuohpEditorExtension } from "~/lexical/extensions";
import { TRANSCRIPT_QUERY, EDIT_STATEMENT_MUTATION, CREATE_STATEMENT_MUTATION, DESTROY_STATEMENT_MUTATION } from "~/queries";
import { style } from "@react-spectrum/s2/style" with { type: "macro" };
import { gql } from "@apollo/client";
import type { HeaderQuery, HeaderQueryVariables } from "./__generated__/interview.$interviewNumber.gql";
import type { TypedDocumentNode } from "@apollo/client";

// FIXME: Constructing URLs for the caption endpoint and the public video URI
// should be a server-side concern (return a Video node, return Caption metadata).
const {
    VITE_AUOHP_API_URI: AUOHP_API_URI,
} = import.meta.env;


export const HEADER_QUERY: TypedDocumentNode<HeaderQuery, HeaderQueryVariables> = gql`
    query Header($interviewNumber: Int!) {
        interview(number: $interviewNumber) {
            interviewee {
                name
            }
        }
    }
`;

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
        /* Seeking moved off selection-change and onto a click here specifically
           (see StatementSeekExtension), so the chrome has to advertise itself as
           the target --- otherwise the only way to discover the gesture is to
           perform it by accident. */
        cursor: pointer;
        user-select: none;

        display: flex;
        flex-direction: column;
        flex-shrink: 0;

        min-width: 6rem;

        font-family: monospace;
        font-size: 0.75rem;
        color: #888;

        transition: color 0.12s ease;

        &:hover {
            color: #333;
        }
    }

    .auohp-statement__content {
        flex: 1;
    }
`;

const EditorContainer = styled.div`
    position: relative;
    overflow-y: auto;
`;

const PageContainer = styled.div`
    overflow: hidden;
    display: grid;
    grid-template-rows: 1fr auto;

    width: 100vw;
    height: 100vh;
`;

const VideoContainer = styled.div`
    display: flex;
    align-items: center;
    justify-content: center;
`;


// The trailing underscore on `$interviewNumber_` is TanStack's de-nesting
// convention: the URL stays `/interview/:n/lexical`, but the route opts OUT of
// rendering inside the Slate route (`interview.$interviewNumber.tsx`), which has
// no <Outlet/>. So this is a sibling under root, not a child --- the Slate route
// stays untouched. The generator maintains the `_` in this path string for us.
export const Route = createFileRoute("/interview/$interviewNumber")({
    component: InterviewEditorPage,
    // FIXME: Handle errors gracefully
    loader: async ({ context: { apolloClient, preloadQuery }, params }) => {
        const interviewNumber = Number.parseInt(params.interviewNumber);
        if (Number.isNaN(interviewNumber)) {
            throw new Error("Invalid interview number");
        }

        const { data: headerQuery } = await apolloClient.query({
            query: HEADER_QUERY,
            variables: {
                interviewNumber,
            },
        });

        const transcriptQuery = preloadQuery(TRANSCRIPT_QUERY, {
            variables: {
                interviewNumber,
            },
        });

        return { transcriptQuery, headerQuery };
    },
    head: ({ params, loaderData }) => {
        const interviewNumber = Number.parseInt(params.interviewNumber);

        const name = loaderData?.headerQuery?.interview?.interviewee?.name ?? "Unknown Interviewee";

        return {
            meta: [
                { title: `#${ interviewNumber } - ${ name } | AUOHP Editor` },
            ],
        };
    },
});


function InterviewEditorPage () {
    const interviewNumber = Number.parseInt(Route.useParams().interviewNumber);
    const { transcriptQuery } = Route.useLoaderData();
    const { data: transcriptData } = useReadQuery(transcriptQuery);

    const [editStatement, editStatementResult] = useMutation(EDIT_STATEMENT_MUTATION, {
        fetchPolicy: "no-cache",
    });
    const [createStatement] = useMutation(CREATE_STATEMENT_MUTATION, {
        fetchPolicy: "no-cache",
    });
    const [destroyStatement] = useMutation(DESTROY_STATEMENT_MUTATION, {
        fetchPolicy: "no-cache",
    });

    const player = useRef<HTMLVideoElement>(null);
    useVideoSync(player);

    const interviewUid = transcriptData?.interview?.uid ?? "";
    const { statements } = transcriptData?.interview.transcript ?? { statements: [] };

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
            destroyStatement,
            createStatement,
            interviewUid,
            statements,
        }), [interviewUid]);

    const statementHash = editStatementResult.data?.editStatement?.newHash;
    const videoUri = transcriptData?.interview?.videos?.[0]?.uri;

    const editorContainerStyle = style({
        backgroundColor: "layer-2",
        padding: 12,
    });

    return (
        <PageContainer>
            <EditorStyle />
            <VideoContainer>
                {
                    videoUri && (
                        <video
                            ref={ player }
                            controls
                            crossOrigin="anonymous"
                            onTimeUpdate={ ev => playhead.timestamp.value = (ev.target as HTMLVideoElement).currentTime }>
                            <source src={ videoUri } type="video/mp4" />
                            <track default key={ statementHash } kind="captions" src={ `${ AUOHP_API_URI }/interview/${ interviewNumber }/vtt` } srcLang="en" label="English" />
                        </video>
                    )
                }
            </VideoContainer>

            <EditorContainer className={ editorContainerStyle }>
                {
                    // The editor does not server-render, and that is a property of
                    // Lexical's architecture rather than a configuration we have got
                    // wrong. Lexical is UNCONTROLLED: the DOM is authoritative, and
                    // decorators are React portals into DOM elements the editor
                    // itself created at runtime (`editor.getElementByKey(nodeKey)`).
                    // There is no DOM during SSR, so there are no portal targets, so
                    // decorators cannot render server-side --- `useDecorators` in
                    // @lexical/react concedes this with its `element !== null` guard.
                    //
                    // The visible symptom was a hydration mismatch on React Spectrum's
                    // `aria-labelledby`/`aria-describedby`, which is misleading twice
                    // over. Spectrum was never at fault (its Provider renders
                    // deterministically --- no `typeof window`, no state, no Suspense),
                    // and the ARIA attributes were not the defect. `useDecorators`
                    // wraps each decorator in <ErrorBoundary><Suspense>, so the server
                    // (zero decorators, document not yet seeded) and the client (N
                    // decorators) built structurally different trees. React 18+'s
                    // `useId` encodes a component's POSITION in the fiber tree, not a
                    // counter, so an extra boundary anywhere above re-keys every id
                    // beneath it. React Aria delegates to `useId`; the ARIA diff was
                    // collateral damage from a structural delta several levels up.
                    //
                    // `ClientOnly` fixes it by making the trees agree rather than by
                    // suppressing the warning: it renders `fallback` on the server AND
                    // on the first client pass, so hydration compares identical trees
                    // and the ids match. The mechanism is `useSyncExternalStore` with
                    // divergent snapshot getters --- `getServerSnapshot` returns false
                    // (used for SSR and for hydration), `getSnapshot` returns true
                    // (every render after) --- which is reconciler-integrated, unlike
                    // the useState/useEffect "mounted" idiom that can paint a frame of
                    // fallback after hydration commits.
                    //
                    // Nothing is lost by not server-rendering here: the editor markup
                    // was being discarded and regenerated on the client anyway.
                }
                <ClientOnly>
                    <LexicalExtensionComposer extension={ extension } />
                </ClientOnly>
            </EditorContainer>
        </PageContainer>
    );
}
