import type { NextPage } from 'next'
import { useEffect, useMemo } from 'react'
import { createEditor } from 'slate'
import { Editable, Slate, withReact } from 'slate-react'

import { Video } from 'components/player'
import { insertCaptionLine, withCaptionLine } from 'editor/captions'
import { Element } from 'editor/element'
import { Transcript, TranscriptRow } from 'editor/transcript'

const initialValue = [
  {
    type: 'paragraph',
    children: [{ text: 'A line of text in a paragraph.' }],
  },
]

const Home: NextPage = () => {
  const editor = useMemo(() => withCaptionLine(withReact(createEditor())), [])

  useEffect(() => {
    insertCaptionLine(editor)
  }, [editor])

  return (
    <>
      <Video />
      <Slate editor={ editor } initialValue={ initialValue }>
        <Editable renderElement={ props => <Element { ...props } /> } />
      </Slate>
      <Transcript>
        <TranscriptRow speaker='Speaker 1' fromTime={ 0 } toTime={ 12 }>
          Still it was a steady pulse of pain midway down his spine. They were dropping,
          losing altitude in a canyon of rainbow foliage, a lurid communal mural that
          completely covered the hull of the Sprawl’s towers and ragged Fuller domes, dim
          figures moving toward him in the dark.
        </TranscriptRow>
        <TranscriptRow speaker='Speaker 2' fromTime={ 12 } toTime={ 24 }>
          The two surviving Founders of Zion were old men, old with the movement of the
          train, their high heels like polished hooves against the gray metal of the
          bright void beyond the chain link. He woke and found her stretched beside him in
          the human system.
        </TranscriptRow>
      </Transcript>
    </>
  )
}

export default Home
