import type { NextPage } from 'next'
import { useEffect, useMemo } from 'react'
import { createEditor } from 'slate'
import { Editable, Slate, withReact } from 'slate-react'

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
      <button className='spectrum-Button spectrum-Button--fill spectrum-Button--accent spectrum-Button--sizeL'>
        <span className='spectrum-Button-label'>Button</span>
      </button>
      <Slate editor={ editor } initialValue={ initialValue }>
        <Editable renderElement={ props => <Element { ...props } /> } />
      </Slate>
    </>
  )
}

export default Home
