import { useEffect, useState } from 'react'

export function useTranscript() {
    const [transcriptJson, setTranscriptJson] = useState(null)

    useEffect(() => {
        fetch('/api/transcript.json')
            .then(async response => {
                if (!response.ok) {
                    throw new Error('Failed to fetch transcript')
                }
                const json = await response.json()
                return json
            })
            .then(setTranscriptJson)
            .catch(console.error)
    }, [])

    return transcriptJson
}
