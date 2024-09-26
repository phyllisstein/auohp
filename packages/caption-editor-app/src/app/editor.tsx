'use client'

import {debounce} from 'lodash-es'
import * as R from 'ramda'
import {Children, useEffect, useMemo} from 'react'
import {createEditor, type Descendant} from 'slate'
import {withHistory} from 'slate-history'
import {
  Editable,
  Slate,
  withReact,
} from 'slate-react'
import {Segment as StyledSegment} from './page-styles'

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
      content: '',
    },
  ],
  type: 'transcript',
}]

const Segment = ({attributes, element, children}) => {
  return (
    <StyledSegment {...attributes} data-testid='statement'>
      <div contentEditable={false} style={{gridRow: '1 / -1', gridColumn: '1'}}>
        <span style={{fontWeight: 'bold'}}>{element.speaker}</span>
      </div>
      <div contentEditable={false} style={{gridRow: '1', gridColumn: '2'}}>
        <span>{element.startTimestamp}</span>
                &nbsp;&ndash;&nbsp;
        <span>{element.endTimestamp}</span>
      </div>
      <div style={{gridRow: '2 / -1', gridColumn: '2 / -1'}}>
        {
          R.intersperse(' ', Children.toArray(children))
        }
      </div>
    </StyledSegment>
  )
}

const Element = props => {
  const {attributes, children, element} = props
  switch (element.type) {
  case 'segment':
    return <Segment {...props} />
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
    if (!editorTranscript || editorTranscript.length < 1 || !editor) return

    const children = [...editor.children]
    children.forEach(node => editor.apply({type: 'remove_node', path: [0], node}))
    editor.apply({type: 'insert_node', path: [0], node: editorTranscript[0]})
    editor.onChange()
  }, [editorTranscript])

  const handleEdit = async value => {
    const isAstChange = editor.operations.some(
      op => 'set_selection' !== op.type,
    )
    if (isAstChange) {
      const content = JSON.stringify(value)
      const makeRequest = debounce(async () => {
        await fetch('/api/transcript.json', {
          method: 'PUT',
          headers: {
            'Content-Type': 'application/json',
          },
          body: content,
        })
      }, 1000)
      makeRequest()
    }
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
