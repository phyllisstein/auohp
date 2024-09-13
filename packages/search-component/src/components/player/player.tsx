import queryString from 'query-string'
import { useEffect, useRef } from 'react'

import { useNeo4jVideo } from 'hooks/interviews'

import './player.scss'

interface PlayerProps {
    interviewNumber: number
}

export function Player({ interviewNumber }: PlayerProps) {
    const player = useRef<HTMLVideoElement>(null)
    const videoURL = useNeo4jVideo(interviewNumber)

    useEffect(() => {
        const currentPlayer = player.current

        const handler = () => {
            if (typeof window === 'undefined' || !currentPlayer) {
                return
            }

            const parsedURL = queryString.parseUrl(window.location.href, { parseNumbers: true, sort: false })
            if (typeof parsedURL.query.timestamp === 'undefined') {
                return
            }

            currentPlayer.currentTime = parsedURL.query.timestamp as unknown as number

            let strippedURL = queryString.stringifyUrl(parsedURL)
            strippedURL = queryString.exclude(strippedURL, ['timestamp'])
            window.history.pushState('', document.title, strippedURL)
        }

        void handler()

        const hashAbortController = new AbortController()
        window.addEventListener('hashchange', handler, {
            signal: hashAbortController.signal,
        })

        return () => {
            hashAbortController.abort()
        }
    })

    return (
        <div className='player-container'>
            <video ref={ player } controls muted playsInline>
                { videoURL && <source src={ videoURL } type='video/mp4' /> }
            </video>
        </div>
    )
}
