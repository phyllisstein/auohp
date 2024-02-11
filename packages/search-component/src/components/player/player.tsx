import queryString from 'query-string'
import { useEffect, useRef } from 'react'

import bordowitz from 'assets/004_gregg_bordowitz_mq.mp4'

import './player.scss'

export function Player() {
    const player = useRef<HTMLVideoElement>(null)

    useEffect(() => {
        const handler = async() => {
            const timestamp = queryString.parse(location.hash, { parseNumbers: true }).timestamp as number
            if (typeof window === 'undefined' || !player.current || !timestamp) {
                return
            }

            player.current.currentTime = timestamp
            await player.current.play()
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
