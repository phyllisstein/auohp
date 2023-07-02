import {
  DecoratorNode,
  type EditorConfig,
  ElementNode,
  type LexicalEditor,
  type NodeKey,
} from 'lexical'
import { ReactNode } from 'react'

export class TranscriptSpeakerNode extends ElementNode {
  static getType () {
    return 'transcript-speaker'
  }

  static clone (node: TranscriptSpeakerNode) {
    return new TranscriptSpeakerNode(node.__key)
  }

  constructor (key: NodeKey) {
    super(key)
  }

  createDOM (config: EditorConfig, editor: LexicalEditor) {
    const element = super.createDOM(config, editor)
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
}
