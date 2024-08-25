import { useMemo } from 'react'

import { useTranscript } from './use-transcript'


const initialValue = [
    {
        children: [
            {
                children: [
                    {
                        type: 'word',
                        startTime: 97.93,
                        endTime: 98.42,
                        word: 'Hello',
                        children: [{ text: 'Hello' }],
                    },
                ],
                endTime: 104.42,
                startTime: 97.93,
                type: 'statement',
                speaker: 'SPEAKER_01',
                content: 'Hello',
            },
        ],
        type: 'transcript',
    },
]

export function useEditorTranscript() {
    const transcript = useTranscript()
    const transcriptJSON = JSON.parse(transcript)

    const sortedTranscript = useMemo(() => {
        if (!transcriptJSON) {
            return initialValue
        }

        return transcriptJSON.sort((a, b) => b.start - a.start)
    }, [transcriptJSON])

    return sortedTranscript
}
