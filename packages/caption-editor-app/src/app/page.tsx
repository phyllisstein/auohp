'use client'

import { type ReactNode, useMemo, useCallback, useRef, useEffect, useState } from 'react'
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
                attributes: {
                    speaker: 'spk_1',
                },
                children: [
                    {
                        attributes: {
                            endTime: 104.42,
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

interface CaptionProps {
    children: ReactNode
    speaker: string
}

const Caption = ({ children, speaker }) => {
    <div style={{ alignItems: 'stretch', display: 'flex', flexDirection: 'column', gap: '10px', justifyContent: 'center' }}>

    </div>
}

// `attributes` are Slate's builtins; user-defined properties of a node come
// through `element`.
const Element = ({ attributes, children, element }) => {
    switch (element.type) {
    case 'caption':
        return <div { ...attributes } data-testid='caption'>{ children }</div>
    case 'statement':
        return <div { ...element?.attributes } { ...attributes } data-testid='statement'>{ children }</div>
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
