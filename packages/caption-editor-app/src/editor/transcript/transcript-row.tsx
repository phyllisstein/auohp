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

export function TranscriptRow ({ children, speaker, fromTime, toTime }: TranscriptRowProps) {
  return (
    <Row>
      <Speaker>{ speaker }</Speaker>
      <Timestamp>{ fromTime }&ndash;{ toTime }</Timestamp>
      <Transcription>
        { children }
      </Transcription>
    </Row>
  )
}
