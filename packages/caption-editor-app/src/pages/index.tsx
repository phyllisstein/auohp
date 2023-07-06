import type { NextPage } from 'next'
import { useCallback, useEffect, useMemo } from 'react'
import { createEditor, Transforms } from 'slate'
import { Editable, Slate, withReact } from 'slate-react'

import { Video } from 'components/player'
import { insertCaptionLine, withCaptionLine } from 'editor/captions'
import { Element } from 'editor/element'
import { insertTranscript, Transcript, TranscriptRow } from 'editor/transcript'
import { withTranscript, withTranscriptRow } from 'editor/transcript/slate-plugin'

const initialValue = [{ text: '' }]

const Home: NextPage = () => {
  const editor = useMemo(() => withTranscriptRow(withTranscript(withCaptionLine(withReact(createEditor())))), [])

  useEffect(() => {
    Transforms.insertNodes(
      editor,
      {
        type: 'transcript',
        children: [{ text: '' }],
      },
      {
        at: [editor.children.length],
      },
    )
  }, [editor])

  return (
    <>
      <Video />
      <Slate editor={ editor } initialValue={ initialValue }>
        <Editable renderElement={ props => {
          console.log({ editableElement: { props } })
          return <Element { ...props } />
        } } renderLeaf={ props => {
          console.log({ editableLeaf: { props } })
          return <span { ...props.attributes } data-custom-prop='leaf'>{ props.children }</span>
        } } />
      </Slate>
    </>
  )
}

export default Home
