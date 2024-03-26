import queryString from 'query-string'
import { useEffect, useRef } from 'react'

import { useNeo4jVideo } from 'hooks/interviews'

import './player.scss'

export function Player() {
    const player = useRef<HTMLVideoElement>(null)

    useEffect(() => {
        const currentPlayer = player.current

        const handler = async() => {
            if (typeof window === 'undefined' || !currentPlayer) {
                return
            }

            const timestamp = queryString.parse(location.hash, { parseNumbers: true })?.timestamp as number
            if (!timestamp) {
                return
            }

            currentPlayer.currentTime = timestamp
            window.history.pushState('', document.title, window.location.pathname + window.location.search)
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
                <source src={ bordowitz } type='video/mp4' />
            </video>
        </div>
    )
}
