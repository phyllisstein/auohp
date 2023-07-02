import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext'
import {
  DecoratorNode,
  type EditorConfig,
  type LexicalEditor,
  type NodeKey,
  LexicalCommand,
  createCommand,
  $insertNodes,
} from 'lexical'
import { ReactNode, useEffect } from 'react'

export const CREATE_SPEAKER_COMMAND: LexicalCommand<string> = createCommand('CREATE_SPEAKER_COMMAND')

export class TranscriptSpeakerNode extends DecoratorNode<unknown> {
  static getType () {
    return 'transcript-speaker'
  }

  static clone (node: TranscriptSpeakerNode) {
    return new TranscriptSpeakerNode(node.__key)
  }

  constructor (key?: NodeKey) {
    super(key)
  }

  createDOM (config: EditorConfig, editor: LexicalEditor) {
    const element = document.createElement('div')
    element.classList.add('transcript-speaker-node')
    return element
  }

  decorate (): ReactNode {
    return (
      <div className='transcript-speaker-decoration'>
        You did the thing!
      </div>
    )
  }
}

export function TranscriptSpeakerPlugin () {
  const [editor] = useLexicalComposerContext()
  useEffect(() => {
    const removeListener = editor.registerCommand(
      CREATE_SPEAKER_COMMAND,
      () => {
        editor.update(() => {
          const speakerNode = new TranscriptSpeakerNode()
          $insertNodes([speakerNode])
        })
        return true
      },
      0,
    )

    return () => removeListener()
  })

  useEffect(() => {
    editor.dispatchCommand(CREATE_SPEAKER_COMMAND, 'test')
  })

  return null
}
