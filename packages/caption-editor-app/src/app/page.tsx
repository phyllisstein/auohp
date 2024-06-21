'use client'

import { useMemo, useCallback, useRef, useEffect, useState } from 'react'
import { Editor, Transforms, Range, createEditor, type Descendant } from 'slate'
import { withHistory } from 'slate-history'
import {
    Slate,
    Editable,
    ReactEditor,
    withReact,
    useSelected,
    useFocused,
} from 'slate-react'
import { Header } from './page-styles'

const initialValue: Descendant[] = [
    {
        children: [
            {
                children: [
                    {
                        attributes: {
                            endTime: 104.42,
                            speaker: 'spk_1',
                            startTime: 97.93,
                        },
                        children: [{ text: 'Statement statement statement.' }],
                        type: 'statement',
                    },
                ],
                type: 'caption',
            },
        ],
    },
]

// `attributes` are Slate's builtins; user-defined properties of a node come
// through `element`.
const Element = ({ attributes, children, element }) => {
    switch (element.type) {
    case 'caption':
    case 'statement':
        return <div { ...element?.attributes } { ...attributes } id='caption'>{ children }</div>
    default:
        return <div { ...attributes }>{ children }</div>
    }
}

const renderElement = props => <Element { ...props } />
const renderLeaf = ({ attributes, children, leaf }) => <span { ...attributes }>{ children }</span>

const withCaptions = (editor) => {}


export default function Page() {
    const editor = useMemo(
        () => withReact(withHistory(createEditor())),
        [],
    )

    return (
        <>
            <Slate
                editor={ editor }
                initialValue={ initialValue }>
                <Editable placeholder='Text...' renderElement={ renderElement } renderLeaf={ renderLeaf } />
            </Slate>
        </>
    )
}
