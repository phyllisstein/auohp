import { useEffect, useState } from 'react'

import { useNeo4j } from 'hooks/infrastructure'

export function useNeo4jVideo(interviewNumber: number) {
    const driver = useNeo4j('bolt://localhost:7687', 'neo4j', 'auohpauohp')
    const [videoURL, setVideoURL] = useState<string>(null)

    useEffect(() => {
        if (!driver) {
            return
        }

        async function loadVideoMetadata() {
            const result = await driver.executeQuery(
                // language=Cypher
                `
                    MATCH (:Interview {number: $interviewNumber}) -[:HAS_VIDEO]-> (video:Video)
                    RETURN video.url AS url
                `, { interviewNumber },
            )

            if (!result.records.length) {
                return
            }

            setVideoURL(result.records[0].get('url'))
        }

        void loadVideoMetadata()
    }, [interviewNumber, driver])

    return videoURL
}
