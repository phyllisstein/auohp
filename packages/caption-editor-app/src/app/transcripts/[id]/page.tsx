'use client'

import styled from 'styled-components'

const Root = styled.div`
    display: grid;
    grid-gap: 1rem;
    grid-template-rows: 1fr 1fr;
    grid-template-columns: 1fr 2fr;
    padding: 1rem;
`

export default function TranscriptPage() {
    return (
        <Root>
            <video>
                <source src='https://dyck.mobi/auohp/035_larry_kramer.mp4' type='video/mp4' />
            </video>
        </Root>
    )
}
