'use client'

import styled from 'styled-components'

const Root = styled.div`
    display: grid;
    grid-gap: 1rem;
    grid-template-rows: 1fr 1fr;
    grid-template-columns: 1fr 2fr;
    width: 100%;
    height: 100vh;
`
const Player = styled.div`
    grid-row: 1;
    grid-column: 2;

    & > video {
        width: 100%;
        height: 100%;
    }
`

const TranscriptTrack = styled.div`
    grid-row: 1 / -1;
    grid-column: 1;

    background-color: ${ ({ theme }) => theme.palette.css.celery800 };
`


export default function TranscriptPage() {
    return (
        <Root>
            <TranscriptTrack>
                <h1>Transcript</h1>
                { /* Transcript goes here */ }
            </TranscriptTrack>
            <Player>
                <video controls crossOrigin='anonymous' preload='metadata'>
                    <source src='https://dyck.mobi/auohp/035_larry_kramer.mp4' type='video/mp4' />
                    <track kind='captions' src='/api/transcript' srcLang='en' />
                </video>
            </Player>
        </Root>
    )
}
