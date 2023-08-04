import type { NextPage } from 'next'
import { useCallback, useEffect, useMemo } from 'react'
import { createEditor, Text, Transforms } from 'slate'
import { Editable, Slate, withReact } from 'slate-react'

import { Video } from 'components/player'
import { insertCaptionLine, withCaptionLine } from 'editor/captions'
import { Element } from 'editor/element'
import { insertTranscript, Transcript, TranscriptRow } from 'editor/transcript'
import { withTranscript } from 'editor/transcript/slate-plugin'

const initialValue = [
  {
    type: 'transcript',
    children: [
      {
        type: 'transcript-row',
        children: [{ text: '' }],
        fromTime: '00:00:00.000',
        toTime: '00:00:10.000',
        speaker: 'Speaker 1',
      },
      {
        type: 'transcript-row',
        children: [{ text: '' }],
        fromTime: '00:00:12.000',
        toTime: '00:00:22.000',
        speaker: 'Speaker 2',
      },
    ],
  },
]

const Home: NextPage = () => {
  const editor = useMemo(
    () => withTranscript(withCaptionLine(withReact(createEditor()))),
    [],
  )
  const renderElement = useCallback(props => <Element { ...props } />, [])
  const renderLeaf = useCallback(
    props => <span { ...props.attributes }>{ props.children }</span>,
    [],
  )

  return (
    <>
      <Video />
      <Slate editor={ editor } initialValue={ initialValue }>
        <Editable renderElement={ renderElement } renderLeaf={ renderLeaf } />
      </Slate>
    </>
  )
}

export default Home
