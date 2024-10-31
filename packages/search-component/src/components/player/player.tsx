import queryString from 'query-string'
import { useEffect, useRef } from 'react'

import { useNeo4jVideo } from 'hooks/interviews'

import './player.scss'

interface PlayerProps {
  url: string
}

export function Player({ url }: PlayerProps) {
  const player = useRef<HTMLVideoElement>(null)
  const video = useNeo4jVideo(url)
  const hasSetTimestamp = useRef<boolean>(false)

  useEffect(() => {
    const currentPlayer = player.current

    const handler = () => {
      if (typeof window === 'undefined' || !currentPlayer || hasSetTimestamp.current) {
        return
      }

      const parsedURL = queryString.parseUrl(window.location.href)
      const localStorageTimestamp = localStorage.getItem('last-timestamp')

      currentPlayer.currentTime
        = typeof parsedURL.query.timestamp === 'string'
          ? Number.parseFloat(parsedURL.query.timestamp)
          : localStorageTimestamp !== null
            ? Number.parseFloat(localStorageTimestamp)
            : 0

      const strURL = queryString.stringifyUrl(parsedURL)
      const strippedURL = queryString.exclude(strURL, ['timestamp'])
      if (window.location.href !== strippedURL) {
        hasSetTimestamp.current = true
        window.history.replaceState('', document.title, strippedURL)
      }
    }

    void handler()
  }, [player])

  useEffect(() => {
    if (typeof window === 'undefined') return

    const currentPlayer = player.current

    const handleBeforeUnload = () => {
      if (currentPlayer) {
        const currentTime = currentPlayer.currentTime
        localStorage.setItem('last-timestamp', currentTime.toString())
      }
    }

    const unloadAbortController = new AbortController()
    window.addEventListener('beforeunload', handleBeforeUnload, {
      signal: unloadAbortController.signal,
    })

    return () => {
      unloadAbortController.abort()
    }
  }, [player])

  return (
    <div className='player-container'>
      <video ref={ player } controls playsInline crossOrigin='anonymous'>
        { video && <source src={ video.video.url } type='video/mp4' /> }
        { video && <track default src={ video.vtt.url } kind='subtitles' srcLang='en' /> }
      </video>
    </div>
  )
}
