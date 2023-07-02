import { LexicalComposer } from '@lexical/react/LexicalComposer'
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext'
import { ContentEditable } from '@lexical/react/LexicalContentEditable'
import LexicalErrorBoundary from '@lexical/react/LexicalErrorBoundary'
import { HistoryPlugin } from '@lexical/react/LexicalHistoryPlugin'
import { OnChangePlugin } from '@lexical/react/LexicalOnChangePlugin'
import { PlainTextPlugin } from '@lexical/react/LexicalPlainTextPlugin'
import type { NextPage } from 'next'
import { useEffect } from 'react'

import { CREATE_SPEAKER_COMMAND, TranscriptSpeakerNode, TranscriptSpeakerPlugin } from '../plugins/transcript-speaker-node'

const Home: NextPage = () => {
  const initialConfig = {
    namespace: 'auohp',
    nodes: [
      TranscriptSpeakerNode,
    ],
    onError: console.error,
  }

  return (
    <LexicalComposer initialConfig={ initialConfig }>
      <div>
        <PlainTextPlugin
          contentEditable={ <ContentEditable /> }
          ErrorBoundary={ LexicalErrorBoundary }
          placeholder={ () => <span>Type something...</span> } />
        <HistoryPlugin />
        <TranscriptSpeakerPlugin />
      </div>
    </LexicalComposer>
  )
}

export default Home
