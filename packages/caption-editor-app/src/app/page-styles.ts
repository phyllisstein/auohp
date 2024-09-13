'use client'

import styled from 'styled-components'

export const Header = styled.h1`
    margin: 0;
    padding: 1rem;

    color: red;
    font-size: 3rem;
`

export const Segment = styled.div`
    display: grid;
    grid-template-rows: auto;
    grid-template-columns: 1fr 4fr;
    gap: 0.5rem;
`


export const Container = styled.main`
    display: grid;
    grid-template-rows: auto;
    grid-template-columns: 4fr 3fr;
    gap: 0.5rem;
    margin: 1rem;
`

export const VideoColumn = styled.div`
    position: relative;
    grid-column: 2
`

export const Video = styled.video`
    width: 100%;
    height: auto;
`

export const EditorColumn = styled.div`
    grid-column: 1;
`
