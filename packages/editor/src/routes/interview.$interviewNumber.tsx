import { createFileRoute } from "@tanstack/react-router";
import { useCallback, useEffect, useRef, useMemo } from "react";
import { type BaseEditor, createEditor, Editor } from "slate";
import { Slate, Editable, type ReactEditor, withReact } from "slate-react";
import { withHistory } from "slate-history";
import { graphql } from "@/gql";
import { useMutation, useReadQuery } from "@apollo/client/react";
import { flow, debounce, throttle } from "es-toolkit/function";
import { useVirtualizer } from "@tanstack/react-virtual";
import { signal, useSignalEffect, createModel } from "@preact/signals-react";


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


type BaseText = { text: string };


interface StatementNode {
    type: string;
    uid: string;
    startTime: number | null | undefined;
    endTime: number | null | undefined;
    children: Array<BaseText>;
}

interface TranscriptNode {
    type: string;
    uid: string;
    children: Array<StatementNode>;
}


declare module "slate" {
    interface CustomTypes {
        Editor: BaseEditor & ReactEditor;
        Element: StatementNode | TranscriptNode;
        Text: BaseText;
    }
}

const Playhead = createModel(() => {
    const seek = signal<number>(0);
    const timestamp = signal<number>(0);

    return { seek, timestamp };
});


const playhead = new Playhead();


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


const DefaultElement = props => {
    return <div { ...props.attributes }>{ props.children }</div>;
};


const StatementElement = ({ attributes, children, element }) => {
    const handleClick = () => {
        playhead.seek.value = element.startTime;
        console.debug(`Clicked on statement: ${ element.uid } (${ element.startTime })`);
    };

    return (
        <div key={ element.uid } onClick={ handleClick } { ...attributes }>
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


const TranscriptElement = ({ attributes, children, element }) => {
    const parentRef = useRef<null | HTMLElement>(null);
    const lastIndex = useRef(0);

    const childVirtualizer = useVirtualizer({
        count: element.children.length,
        estimateSize: () => 64,
        getScrollElement: () => parentRef.current,
        useScrollendEvent: true,
        getItemKey: index => element.children[index].uid,
        onChange: (instance, sync) => {
            const items = instance.getVirtualItems();

            const item = items.find(item => item.start >= instance.scrollOffset);
            const index = item ? item.index : -1;

            if (instance.isScrolling || index === lastIndex.current || index === -1) {
                return;
            }

            playhead.seek.value = element.children[index].startTime;
            lastIndex.current = index;

            console.debug(`Virtualizer scrolled to statement: ${ element.children[index].uid } (${ element.children[index].startTime })`);
        },
    });

    const setRefs = useCallback(node => {
        parentRef.current = node;
        attributes.ref(node);
    }, []);

    useSignalEffect(() => {
        const index = element.children.findIndex(node => node.startTime <= playhead.timestamp.value && playhead.timestamp.value < node.endTime);

        if (childVirtualizer.isScrolling || lastIndex.current === index) {
            return;
        }

        lastIndex.current = index;

        console.debug(`childVirtualizer scrolling to statement: ${ element.children[index].uid } (${ element.children[index].startTime })`);
        childVirtualizer.scrollToIndex(index, {
            align: "center",
            behavior: "smooth",
        });
    });


    return (
        <div { ...attributes } ref={ setRefs } style={{ height: "400px", overflow: "auto", position: "relative" }}>
            <div style={{ position: "relative", height: `${ childVirtualizer.getTotalSize() }px` }}>
                { childVirtualizer.getVirtualItems().map(virtualItem => {
                    const component = children[virtualItem.index];
                    return (
                        <div
                            key={ virtualItem.key }
                            style={{
                                height: `${ virtualItem.size }px`,
                                transform: `translateY(${ virtualItem.start }px)`,
                                position: "absolute",
                                left: 0,
                                top: 0,
                                width: "100%",
                            }}>
                            { component }
                        </div>
                    );
                }) }
            </div>
        </div>
    );
};


const renderElement = props => {
    switch (props.element.type) {
        case "statement":
            return <StatementElement { ...props } />;
        case "transcript":
            return <TranscriptElement { ...props } />;
        default:
            return <DefaultElement { ...props } />;
    }
};


const withPersistence = editStatement => (editor: Editor) => {
    editStatement = debounce(editStatement, 1_000);
    const { onChange } = editor;

    editor.onChange = args => {
        onChange(args);

        if (args?.operation.type?.includes("text")) {
            const [parent] = Editor.parent(editor, args.operation.path);

            editStatement({
                variables: {
                    uid: parent.uid,
                    text: parent.children[0].text,
                },
                onCompleted: data => {
                    console.debug(`Edit completed for statement ${ data.editStatement.uid }:`, data.editStatement);
                },
            });
        }
    };
    return editor;
};


function Page() {
    const interviewNumber = Number.parseInt(Route.useParams().interviewNumber);
    const { transcriptQueryRef } = Route.useLoaderData();
    const { data: transcriptData } = useReadQuery(transcriptQueryRef);

    const [editStatement, { data: editStatementData }] = useMutation(EDIT_STATEMENT_MUTATION);

    const withPlugins = flow(
        withReact,
        withHistory,
        withPersistence(editStatement),
    );
    const editor = useMemo<Editor>(() => withPlugins(createEditor()), []);

    const player = useRef<HTMLVideoElement>(null);
    useSignalEffect(() => {
        !!player.current && (player.current.currentTime = playhead.seek.value);
    });

    const { statements, interview, uid: transcriptUid } = transcriptData.interviewTranscript;
    const statementNodes = statements.map(statement => ({
        type: "statement",
        uid: statement.uid,
        startTime: statement.startTime,
        endTime: statement.endTime,
        children: [
            { text: statement.text },
        ],
    }));

    const transcriptNode = {
        type: "transcript",
        uid: transcriptUid,
        children: statementNodes,
    };

    return (
        <div>
            <video ref={ player } controls crossOrigin="anonymous" onTimeUpdate={ ev => playhead.timestamp.value = ev.target.currentTime }>
                <source src={ `${ AUOHP_PUBLIC }/videos/${ interviewNumber }.mp4` } type="video/mp4" />
                {
                    interview.uid && <track kind="captions" src={ `${ AUOHP_API_URI }/interview/${ interview.uid }/vtt` } srcLang="en" label="English" />
                }
            </video>
            <Slate editor={ editor } initialValue={ [transcriptNode] } key={ interview.uid }>
                <Editable renderElement={ renderElement } />
            </Slate>
        </div>
    );
}
