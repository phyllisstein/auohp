import type { NextPage } from 'next'
import { useEffect, useMemo } from 'react'
import { createEditor } from 'slate'
import { Editable, Slate, withReact } from 'slate-react'

import { Video } from 'components/player'
import { insertCaptionLine, withCaptionLine } from 'editor/captions'
import { Element } from 'editor/element'

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
    </>
  )
}

export default Home
