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
}

const Caption: FunctionComponent<CaptionProps> = ({ attributes, element, children }) => {
    return (
        <div { ...attributes } data-testid='caption' style={{ alignItems: 'stretch', display: 'flex', flexDirection: 'column', gap: '10px', justifyContent: 'center' }}>
            <div contentEditable={ false } style={{ display: 'flex', gap: '10px' }}>
                <span style={{ fontWeight: 'bold' }}>{ element.speaker }</span>
            </div>
            { children }
        </div>
    )
}

const Statement = ({ attributes, element, children,  }) => {
    return (
        <div { ...attributes } data-testid='statement'>
            <div contentEditable={ false }>
                <span>{ element.startTime }</span>
                —
                <span>{ element.endTime }</span>
            </div>
            { children }
        </div>
    )
}

const Element = props => {
    const { attributes, children, element } = props
    switch (element.type) {
    case 'caption':
        return <Caption { ...props } />
    case 'statement':
        return <Statement { ...props } />
    default:
        return <div { ...attributes }>{ children }</div>
    }
}

const renderElement = props => <Element { ...props } />
const renderLeaf = ({ attributes, children, leaf }) => <span { ...attributes }>{ children }</span>

const withCaptions = editor => {}


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
