import queryString from "query-string";
import { useEffect, useRef } from "react";
import { gql } from "@apollo/client";
import { useQuery } from "@apollo/client/react";
import type { TypedDocumentNode } from "@graphql-typed-document-node/core";
import type { PlayerInterviewQuery, PlayerInterviewQueryVariables } from "~/gql/schema";

export const PLAYER_INTERVIEW_QUERY: TypedDocumentNode<PlayerInterviewQuery, PlayerInterviewQueryVariables> = gql`
    query PlayerInterview($interviewNumber: Int!) {
        interview(number: $interviewNumber) {
            videos {
                uri
            }
        }
    }
`;

interface PlayerProps {
    interviewNumber: number;
}

export function Player ({ interviewNumber }: PlayerProps) {
    const player = useRef<HTMLVideoElement>(null);
    const hasSetTimestamp = useRef<boolean>(false);
    const { data } = useQuery(PLAYER_INTERVIEW_QUERY, {
        variables: { interviewNumber },
    });

    const videoUri = data?.interview?.videos?.[0]?.uri;

    useEffect(() => {
        const currentPlayer = player.current;

        const handler = () => {
            if (typeof window === "undefined" || !currentPlayer || hasSetTimestamp.current) {
                return;
            }

            const parsedURL = queryString.parseUrl(window.location.href);
            const localStorageTimestamp = localStorage.getItem("last-timestamp");

            currentPlayer.currentTime =
                typeof parsedURL.query.timestamp === "string"
                    ? Number.parseFloat(parsedURL.query.timestamp)
                    : localStorageTimestamp !== null
                        ? Number.parseFloat(localStorageTimestamp)
                        : 0;

            const strURL = queryString.stringifyUrl(parsedURL);
            const strippedURL = queryString.exclude(strURL, ["timestamp"]);
            if (window.location.href !== strippedURL) {
                hasSetTimestamp.current = true;
                window.history.replaceState("", document.title, strippedURL);
            }
        };

        void handler();
    }, [player, videoUri]);

    useEffect(() => {
        if (typeof window === "undefined") return;

        const currentPlayer = player.current;

        const handleBeforeUnload = () => {
            if (currentPlayer) {
                const currentTime = currentPlayer.currentTime;
                localStorage.setItem("last-timestamp", currentTime.toString());
            }
        };

        const unloadAbortController = new AbortController();
        window.addEventListener("beforeunload", handleBeforeUnload, {
            signal: unloadAbortController.signal,
        });

        return () => {
            unloadAbortController.abort();
        };
    }, [player, videoUri]);

    return (
        <div className="player-container">
            { videoUri && (
                <video ref={ player } controls crossOrigin="anonymous" className="player">
                    <source src={ videoUri } type="video/mp4" />
                    <track default kind="captions" srcLang="en" label="English" src={ `https://api.auohp.localhost/interview/${ interviewNumber }/vtt` } />
                </video>
            ) }
        </div>
    );
}
