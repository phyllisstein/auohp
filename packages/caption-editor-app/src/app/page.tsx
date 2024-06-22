'use client'

import { type FunctionComponent, type ReactNode, useMemo, useCallback, useRef, useEffect, useState } from 'react'
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
                        children: [{ text: 'Statement statement statement.' }],
                        endTime: 104.42,
                        startTime: 97.93,
                        type: 'statement',
                    },
                ],
                speaker: 'spk_1',
                type: 'caption',
            },
        ],
    },
]

interface CaptionProps {
    children: ReactNode
    speaker: string
}

const Caption: FunctionComponent<CaptionProps> = ({ children, speaker, ...props }) => {
    return (
        <div { ...props } data-testid='caption' style={{ alignItems: 'stretch', display: 'flex', flexDirection: 'column', gap: '10px', justifyContent: 'center' }}>
            <div style={{ display: 'flex', gap: '10px' }}>
                <span style={{ fontWeight: 'bold' }}>{ speaker }</span>
                <span>{ children }</span>
            </div>
        </div>
    )
}

const Statement = ({ children, endTime, startTime, ...props }) => {
    return (
        <div { ...props } data-testid='statement'>
            <div contentEditable={ false }>
                <span>{ startTime }</span>
                —
                <span>{ endTime }</span>
            </div>
            { children }
        </div>
    )
}

// `attributes` are Slate's builtins; user-defined properties of a node come
// through `element`.
const Element = ({ attributes, children, element }) => {
    debugger
    switch (element.type) {
    case 'caption':
        return <Caption { ...attributes } speaker={ element.speaker }>{ children }</Caption>
    case 'statement':
        return <Statement { ...attributes } endTime={ element.endTime } startTime={ element.startTime }>{ children }</Statement>
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
