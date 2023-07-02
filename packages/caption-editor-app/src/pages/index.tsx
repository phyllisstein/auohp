import { LexicalComposer } from '@lexical/react/LexicalComposer'
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext'
import { ContentEditable } from '@lexical/react/LexicalContentEditable'
import LexicalErrorBoundary from '@lexical/react/LexicalErrorBoundary'
import { HistoryPlugin } from '@lexical/react/LexicalHistoryPlugin'
import { OnChangePlugin } from '@lexical/react/LexicalOnChangePlugin'
import { PlainTextPlugin } from '@lexical/react/LexicalPlainTextPlugin'
import type { NextPage } from 'next'
import { useEffect } from 'react'

import { TranscriptSpeakerNode } from 'plugins/transcript-speaker-node'

const Home: NextPage = () => {
  const initialConfig = {
    namespace: 'auohp',
    plugins: [
      TranscriptSpeakerNode,
    ],
  }

  return (
    <LexicalComposer initialConfig={ initialConfig }>
      <div>
        <PlainTextPlugin
          contentEditable={ <ContentEditable /> }
          ErrorBoundary={ LexicalErrorBoundary }
          placeholder={ () => <span>Type something...</span> } />
        <HistoryPlugin />
      </div>
    </LexicalComposer>
  )
}

export default Home
