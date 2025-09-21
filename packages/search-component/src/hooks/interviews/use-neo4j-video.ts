import { useEffect, useState } from "react";

import { useNeo4j } from "hooks/infrastructure";

export interface Video {
    uid: string;
    url: string;
}

export interface VTT {
    uid: string;
    url: string;
}

interface Neo4jVideo {
    video: Video;
    vtt: VTT;
}

export function useNeo4jVideo(url: string) {
    const driver = useNeo4j();
    const [videoURL, setVideoURL] = useState<Neo4jVideo>(null);

    useEffect(() => {
        if (!driver) {
            return;
        }

        async function loadVideoMetadata() {
            const result = await driver.executeQuery(
                // language=Cypher
                `
                    MATCH (video:Video {url: $url}) -[:HAS_CAPTIONS]-> (vtt)
                    RETURN video, vtt
                `, { url },
            );

            if (!result.records.length) {
                return;
            }

            setVideoURL({
                video: result.records[0].get("video").properties,
                vtt: result.records[0].get("vtt").properties,
            });
        }

        void loadVideoMetadata();
    }, [url, driver]);

    return videoURL;
}
