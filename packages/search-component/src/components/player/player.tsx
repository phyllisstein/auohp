import queryString from 'query-string'
import { useEffect, useRef } from 'react'

import './player.scss'

interface PlayerProps {
    videoURL: string
}

export function Player({ videoURL }: PlayerProps) {
    const player = useRef<HTMLVideoElement>(null)

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
