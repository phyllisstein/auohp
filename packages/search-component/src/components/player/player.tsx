import queryString from 'query-string'
import { useEffect, useRef, useState } from 'react'
import ReactPlayer from 'react-player'

import { useQueryTimestamp } from './use-query-timestamp'

export function Player () {
    const player = useRef<HTMLVideoElement>(null)
    const [timestamp, setTimestamp] = useState(0)

    const handleTimestampChange = async () => {
        debugger
        const urlTimestamp = queryString.parse(location.hash, { parseNumbers: true }).timestamp
        if (player.current == null || window == null || urlTimestamp == null) {
            return
        }

        let bareURL = queryString.stringify({
            ...queryString.parse(location.hash),
            timestamp: urlTimestamp,
        })

        console.log('bareURL', bareURL)
        console.log(urlTimestamp)

        player.current.currentTime = urlTimestamp
        await player.current.play()
        setTimestamp(urlTimestamp)

        history.pushState(null, '', `#${ bareURL }`)
    }

    useEffect(() => {
        window.addEventListener('hashchange', handleTimestampChange)
        window.addEventListener('urlchange', handleTimestampChange)
        void handleTimestampChange()

        // return () => window.removeEventListener('hashchange', handleTimestampChange)
    }, [])

    return (
        <div>
            <video ref={ player } controls onDurationChange={ handleTimestampChange }>
                <source src='https://s3.amazonaws.com/act-up-oral-history-resilient-reserve-4710/004_gregg_bordowitz_mq.mp4' type='video/mp4' />
            </video>
        </div>
    )
}
