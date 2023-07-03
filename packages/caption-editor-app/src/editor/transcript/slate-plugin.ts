import { Transforms } from 'slate'

export function withTranscript (editor) {
  const { isVoid } = editor

  editor.isVoid = element => {
    return element.type === 'transcript' ? true : isVoid(element)
  }

  return editor
}

export function insertTranscript (editor) {
  const transcript = {
    type: 'transcript',
    children: [{ text: '' }],
  }

  Transforms.insertNodes(editor, transcript)
}
