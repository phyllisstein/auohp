import { useEffect, useState } from 'react'

export function useTranscript() {
    const [transcriptJson, setTranscriptJson] = useState(null)

    useEffect(() => {
        fetch('/api/transcript.json')
            .then(response => response.text())
            .then(setTranscriptJson)
    }, [])

    return transcriptJson
}
