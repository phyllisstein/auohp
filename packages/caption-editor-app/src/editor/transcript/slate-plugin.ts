import { Transforms } from 'slate'

export function withTranscript (editor) {
  const { isVoid } = editor

  editor.isVoid = element => {
    return (element.type === 'transcript' || element.type === 'transcript-row') ? false : isVoid(element)
  }

  return editor
}

export function insertTranscript (editor) {
  const transcript = {
    type: 'transcript',
    children: [
      {
        type: 'transcript-row',
        children: [{ text: '' }],
      },
    ],
  }

  Transforms.insertNodes(editor, transcript)
}

export function insertTranscriptRow (editor) {
  const transcriptRow = {
    type: 'transcript-row',
    children: [{ text: '' }],
  }

  Transforms.insertNodes(editor, transcriptRow)
}
