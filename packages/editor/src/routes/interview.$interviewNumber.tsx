import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { type BaseEditor, createEditor, Editor } from "slate";
import { Slate, Editable, type ReactEditor, withReact } from "slate-react";
import { graphql } from "../gql";


// FIXME: Constructing URLs for the caption endpoint and the public video URI
// should be a server-side concern (return a Video node, return Caption metadata).
const {
    VITE_AUOHP_PUBLIC: AUOHP_PUBLIC,
    VITE_AUOHP_API_URI: AUOHP_API_URI,
} = import.meta.env;


const TRANSCRIPT_QUERY = graphql(`
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

interface WordElement {
    type: "word";
    startTime: number;
    endTime: number;
    children: BaseText[];
}

interface StatementElement {
    type: "statement";
    uid: string;
    startTime: number;
    endTime: number;
    children: Array<WordElement | BaseText>;
}

declare module "slate" {
    interface CustomTypes {
        Editor: BaseEditor & ReactEditor;
        Element: StatementElement | WordElement;
        Text: BaseText;
    }
}


export const Route = createFileRoute("/interview/$interviewNumber")({
    component: Page,
    // FIXME: Handle errors gracefully
    loader: async ({ context, params }) => {
        const interviewNumber = Number.parseInt(params.interviewNumber);
        if (Number.isNaN(interviewNumber)) {
            throw new Error("Invalid interview number");
        }

        const client = context.apolloClient;

        const { data } = await client.query({
            query: TRANSCRIPT_QUERY,
            variables: { interviewNumber },
        });

        if (!data?.interviewTranscript) {
            throw new Error("Transcript not found");
        }

        const statementData = data.interviewTranscript.statements;
        const statementElements = statementData.map(statement => ({
            type: "statement",
            uid: statement.uid,
            startTime: statement.startTime,
            endTime: statement.endTime,
            children: [
                { text: statement.text },
            ],
        }));

        return { statementElements, interview: data?.interviewTranscript?.interview };
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


const renderElement = props => {
    switch (props.element.type) {
        case "statement":
            return <StatementElement { ...props } />;
        default:
            return <DefaultElement { ...props } />;
    }
};


function Page() {
    const [editor] = useState<Editor>(() => withReact(createEditor()));

    const interviewNumber = Number.parseInt(Route.useParams().interviewNumber);
    const { statementElements, interview } = Route.useLoaderData();

    return (
        <>
            <video controls crossOrigin="anonymous">
                <source src={ `${ AUOHP_PUBLIC }/videos/${ interviewNumber }.mp4` } type="video/mp4" />
                {
                    interview?.uid && <track kind="captions" src={ `${ AUOHP_API_URI }/interview/${ interview.uid }/vtt` } srcLang="en" label="English" />
                }
            </video>
            <Slate editor={ editor } initialValue={ statementElements } key={ interview?.uid }>
                <Editable renderElement={ renderElement } />
            </Slate>
        </>
    );
}
