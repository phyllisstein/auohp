import queryString from 'query-string'
import { useEffect, useRef } from 'react'

import './player.scss'

export function Player () {
    const player = useRef<HTMLVideoElement>(null)

    useEffect(() => {
        const handler = async () => {
            const timestamp = queryString.parse(location.hash, { parseNumbers: true }).timestamp as number
            if (typeof window === 'undefined' || !player.current || !timestamp) {
                return
            }

            player.current.currentTime = timestamp
            await player.current.play()
        }

        void handler()
        window.addEventListener('hashchange', handler)

        return () => {
            window.removeEventListener('hashchange', handler)
        }
    })

    return (
        <div className='player-container'>
            <video ref={ player } controls muted playsInline>
                <source src='https://s3.amazonaws.com/act-up-oral-history-resilient-reserve-4710/004_gregg_bordowitz_mq.mp4' type='video/mp4' />
            </video>
        </div>
    )
}
