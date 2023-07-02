import { LexicalComposer } from '@lexical/react/LexicalComposer'
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext'
import { ContentEditable } from '@lexical/react/LexicalContentEditable'
import LexicalErrorBoundary from '@lexical/react/LexicalErrorBoundary'
import { HistoryPlugin } from '@lexical/react/LexicalHistoryPlugin'
import { OnChangePlugin } from '@lexical/react/LexicalOnChangePlugin'
import { PlainTextPlugin } from '@lexical/react/LexicalPlainTextPlugin'
import type { NextPage } from 'next'
import { useEffect } from 'react'

const Home: NextPage = () => {
  return (
    <LexicalComposer initialConfig={{ namespace: 'auohp' }}>
      <div>
        <PlainTextPlugin
          contentEditable={ <ContentEditable /> }
          errorBoundary={ <LexicalErrorBoundary /> }
          placeholder='Type something...' />
        <HistoryPlugin />
      </div>
    </LexicalComposer>
  )
}

export default Home
