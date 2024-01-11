import { oneLine } from 'common-tags'
import { Descendant, createEditor } from 'slate'
import { Editable, Slate, useSlateStatic, useSlateWithV, withReact } from 'slate-react'
import styled from 'styled-components'

interface TranscriptRowProps {
    children?: React.ReactNode
    speaker?: string
    fromTime?: number
    toTime?: number
}

const Row = styled.div`
    display: grid;
    grid-gap: 0;
    grid-template-areas:
        'speaker timestamp'
        'speaker transcription';
    grid-template-rows: 2rem 1fr;
    grid-template-columns: 1fr 3fr;
`

const Speaker = styled.div`
    grid-area: speaker;
`

const Timestamp = styled.div`
    grid-area: timestamp;
`

const Transcription = styled.div`
    grid-area: transcription;
`

const initialValue: Descendant[] = [
    {
        type: 'paragraph',
        children: [
            {
                text: oneLine`
          Still it was a steady pulse of pain midway down his spine. They were dropping,
          losing altitude in a canyon of rainbow foliage, a lurid communal mural that
          completely covered the hull of the Sprawl’s towers and ragged Fuller domes, dim
          figures moving toward him in the dark.
        `,
            },
        ],
    },
]

export function TranscriptRow({ attributes, children, element }) {
    const { speaker, fromTime, toTime } = element

    return (
        <div { ...attributes }>
            <Row>
                <div contentEditable={ false } style={{ display: 'contents' }}>
                    <Speaker>{ speaker }</Speaker>
                    <Timestamp>{ fromTime }&ndash;{ toTime }</Timestamp>
                </div>
                <Transcription>
                    { children }
                </Transcription>
            </Row>
        </div>
    )
}
