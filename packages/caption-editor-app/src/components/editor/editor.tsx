'use client'

import * as R from 'ramda'
import {useEffect, useMemo} from 'react'
import {createEditor, type Descendant, Path} from 'slate'
import {withHistory} from 'slate-history'
import {
  Editable,
  Slate,
  withReact,
} from 'slate-react'
import {Statement} from './statement'

const EMPTY: Descendant[] = [{
  children: [
    {
      children: [
        {
          type: 'word',
          startTime: 97.93,
          endTime: 98.42,
          word: '',
          children: [{text: ''}],
        },
      ],
      endTime: 104.42,
      startTime: 97.93,
      type: 'segment',
      speaker: 'SPEAKER_01',
      transcription: '',
      uid: '0',
    },
  ],
  type: 'transcript',
}]


const Element = props => {
  const {attributes, children, element} = props
  switch (element.type) {
  case 'statement':
    return <Statement {...props} />
  case 'word':
    return <span {...attributes} data-testid='word'>{children}</span>
  default:
    return <div {...attributes} data-testid='blank-div'>{children}</div>
  }
}

const renderElement = props => <Element {...props} />
const renderLeaf = ({attributes, children}) => <span {...attributes}>{children}</span>

const withCaptions = editor => {}

const withInlines = editor => {
  const {isInline} = editor

  editor.isInline = element => {
    return element.type === 'word' ? true : isInline(element)
  }

  return editor
}

export function Editor({initialContent = EMPTY, editorTranscript}) {
  const editor = useMemo(
    () => withInlines(withReact(withHistory(createEditor()))),
    [],
  )

  useEffect(() => {
    if (!editorTranscript || !editorTranscript.children || !editor) return

    const children = [...editor.children]
    children.forEach(node => editor.apply({type: 'remove_node', path: [0], node}))
    editor.apply({type: 'insert_node', path: [0], node: editorTranscript})
  }, [editorTranscript])

  const handleEdit = async value => {
    console.log({operations: editor.operations})

    const changes = editor.operations.filter(
      op => 'set_selection' !== op.type,
    )
    if (R.isEmpty(changes)) return

    // TODO: Handle and batch multiple changes
    if (changes.length > 1) {
      return
    }
    const change = changes[0]
    const path = Path.parent(change.path)
    const node = value[path[0]].children[path[1]]
    console.log({node, value})

    const res = await fetch('/api/transcript', {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(node),
    })

    if (!res.ok) {
      console.error('Failed to update transcript')
    }

    const json = await res.json()
    console.log(json)
    // editor.apply({type: 'set_node', path, properties: json})
  }

  return (
    <Slate
      editor={editor}
      initialValue={initialContent}
      onChange={handleEdit}>
      <Editable renderElement={renderElement} renderLeaf={renderLeaf} />
    </Slate>
  )
}
