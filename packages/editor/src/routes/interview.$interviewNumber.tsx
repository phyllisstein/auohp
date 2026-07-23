import { createFileRoute } from "@tanstack/react-router";
import { useCallback, useRef, useMemo } from "react";
import { type BaseEditor, createEditor, Editor } from "slate";
import { Slate, Editable, type ReactEditor, withReact } from "slate-react";
import { withHistory } from "slate-history";
import { graphql } from "@/gql";
import { useMutation, useReadQuery } from "@apollo/client/react";
import { flow, debounce } from "es-toolkit/function";
import { useVirtualizer } from "@tanstack/react-virtual";


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


const TranscriptElement = ({ attributes, children, element }) => {
    const parentRef = useRef<null | HTMLElement>(null);
    const childVirtualizer = useVirtualizer({
        count: element.children.length,
        estimateSize: () => 64,
        getScrollElement: () => parentRef.current,
        getItemKey: index => element.children[index].uid,
    });

    const setRefs = useCallback(node => {
        parentRef.current = node;
        attributes.ref(node);
    }, []);

    return (
        <div { ...attributes } ref={ setRefs } style={{ height: "400px", overflow: "auto" }}>
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
                    console.debug("Edit completed:", data);
                },
            });
        }
    };
    return editor;
};


function Page() {
    const interviewNumber = Number.parseInt(Route.useParams().interviewNumber);
    const { transcriptQueryRef } = Route.useLoaderData();
    const { data } = useReadQuery(transcriptQueryRef);

    const [editStatement, { data: editStatementData }] = useMutation(EDIT_STATEMENT_MUTATION);

    const withPlugins = flow(
        withReact,
        withHistory,
        withPersistence(editStatement),
    );
    const editor = useMemo<Editor>(() => withPlugins(createEditor()), []);

    const { statements, interview, uid: transcriptUid } = data.interviewTranscript;
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
        <>
            <video controls crossOrigin="anonymous">
                <source src={ `${ AUOHP_PUBLIC }/videos/${ interviewNumber }.mp4` } type="video/mp4" />
                {
                    interview.uid && <track kind="captions" src={ `${ AUOHP_API_URI }/interview/${ interview.uid }/vtt` } srcLang="en" label="English" />
                }
            </video>
            <Slate editor={ editor } initialValue={ [transcriptNode] } key={ interview.uid }>
                <Editable renderElement={ renderElement } />
            </Slate>
        </>
    );
}
