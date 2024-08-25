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

    // const editorTranscript = useMemo(() => {
    //     if (!transcript || transcript.length === 0) return initialValue

    //     return transcriptJSON.segments.map(segment => {
    //         const captionChildren = segment.words.map(word => ({
    //             type: 'word',
    //             startTime: word.startTime,
    //             endTime: word.endTime,
    //             word: word.word,
    //             children: [{ text: word.word + ' ' }], // FIXME: Terrible hack
    //         }))

    //         return {
    //             startTime: dayjs.duration({ seconds: segment.startTime }).format('H:mm:ss'),
    //             endTime: dayjs.duration({ seconds: segment.endTime }).format('H:mm:ss'),
    //             speaker: segment.speaker,
    //             content: segment.text,
    //             type: 'statement',
    //             children: captionChildren,
    //         }
    //     })
    // }, [transcript])

    return transcriptJSON
}
