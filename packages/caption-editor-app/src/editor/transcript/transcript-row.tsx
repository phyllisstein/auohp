import { Descendant } from 'slate'
import { Editable, Slate, useSlateStatic, useSlateWithV } from 'slate-react'
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
        text: `
          Still it was a steady pulse of pain midway down his spine. They were dropping,
          losing altitude in a canyon of rainbow foliage, a lurid communal mural that
          completely covered the hull of the Sprawl’s towers and ragged Fuller domes, dim
          figures moving toward him in the dark.
        `,
      },
    ],
  },
]

export function TranscriptRow ({ attributes, children, element }) {
  console.log('<TranscriptRow />', { attributes, children, element })
  const { speaker, fromTime, toTime } = element ?? {}
  const editor = useSlateStatic()

  return (
    <div { ...attributes }>
      <Row>
        <Speaker>{ speaker }</Speaker>
        <Timestamp>{ fromTime }&ndash;{ toTime }</Timestamp>
        <Transcription>
          <Slate editor={ editor } initialValue={ initialValue }>
            <Editable />
          </Slate>
        </Transcription>
      </Row>
      { children }
    </div>
  )
}
