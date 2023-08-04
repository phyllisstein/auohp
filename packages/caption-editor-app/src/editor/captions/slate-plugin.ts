import { Transforms } from 'slate'

export function withCaptionLine (editor) {
  const { isVoid } = editor

  editor.isVoid = element => {
    return element.type === 'caption-line' ? true : isVoid(element)
  }

  return editor
}

export function insertCaptionLine (editor) {
  const captionLine = {
    type: 'caption-line',
    children: [{ text: '' }],
  }

  Transforms.insertNodes(editor, captionLine)
}
