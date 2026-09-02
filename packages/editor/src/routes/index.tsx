import { gql } from "@apollo/client";
import React, { useState } from "react";
import { createFileRoute, createLink } from "@tanstack/react-router";
import { useReadQuery } from "@apollo/client/react";
import type { TypedDocumentNode } from "@apollo/client";
import type { ListInterviewsQuery, ListInterviewsQueryVariables } from "./__generated__/index.gql";
import { Route as InterviewRoute } from "./interview.$interviewNumber";
import { Route as SearchRoute } from "./search/route";
import { style, baseColor } from "@react-spectrum/s2/style" with { type: "macro" };
import styled from "styled-components";

export const LIST_INTERVIEWS_QUERY: TypedDocumentNode<ListInterviewsQuery, ListInterviewsQueryVariables> = gql`
    query ListInterviews {
        interviews {
            number
            interviewee {
                name
            }
        }
    }
`;

const LinkComponent = styled.a`
`;

const BasicLinkComponent = React.forwardRef<HTMLAnchorElement, React.ComponentProps<"a">>(
    (props, ref) => {
        const [isHovered, setIsHovered] = useState(false);
        const [isPressed, setIsPressed] = useState(false);
        const [isFocusVisible, setIsFocusVisible] = useState(false);

        const cn = style({
            color: baseColor("accent-800"),
            transition: "colors",
            cursor: "pointer",
        });

        return (
            <a
                { ...props }
                title={ props.title }
                ref={ ref }
                className={ cn({ isHovered, isPressed, isFocusVisible }) }
                onFocus={ () => setIsFocusVisible(true) }
                onBlur={ () => setIsFocusVisible(false) }
                onMouseDown={ () => setIsPressed(true) }
                onMouseUp={ () => setIsPressed(false) }
                onMouseOver={ () => setIsHovered(true) }
                onMouseOut={ () => setIsHovered(false) } />
        );
    },
);

const StackLink = createLink(BasicLinkComponent);

export const Route = createFileRoute("/")({
    component: IndexPage,
    loader: async ({ context: { preloadQuery }, params }) => {
        const listInterviewsQuery = preloadQuery(LIST_INTERVIEWS_QUERY);

        return { listInterviewsQuery };
    },
});

function IndexPage () {
    const { listInterviewsQuery } = Route.useLoaderData();
    const { data: interviewsData } = useReadQuery(listInterviewsQuery);
    const interviews = interviewsData?.interviews ?? [];

    return (
        <div className={ style({ backgroundColor: "layer-2", height: "full", padding: "edge-to-text", margin: "text-to-control", borderRadius: "sm" }) }>
            <div className={ style({ width: "max", height: "max" }) }>
                <h3>Interviews</h3>
                <ul>
                    { interviews.map(interview => (
                        <li key={ interview.number }>
                            <StackLink to={ InterviewRoute.to } params={{ interviewNumber: `${ interview.number }` }} title={ `Interview ${ interview.number }` }>
                                #{ interview.number } - { interview.interviewee.name }
                            </StackLink>
                        </li>
                    )) }
                </ul>
                <h3>Search</h3>
                <StackLink to={ SearchRoute.to } title="Search">
                    Search
                </StackLink>
            </div>
        </div>
    );
}
