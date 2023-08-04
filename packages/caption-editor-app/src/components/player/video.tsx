import { useEffect, useRef, useState } from 'react'
import { useRecoilState } from 'recoil'

import * as videoState from 'state/player'

interface VideoProps {
  children?: React.ReactNode
}

export function Video ({ children }: VideoProps) {
  const [videoRef, setVideo] = useState<HTMLVideoElement>()
  const [playhead, setPlayhead] = useRecoilState(videoState.playhead)

  useEffect(() => {
    console.log('useEffect')
    const vr = videoRef

    if (!vr) {
      return
    }

    const controller = new AbortController()

    vr.addEventListener(
      'timeupdate',
      event => {
        setPlayhead(playhead => ({
          ...playhead,
          currentTime: event.target.currentTime,
        }))
      },
      { signal: controller.signal },
    )

    vr.addEventListener(
      'playing',
      () => {
        setPlayhead(playhead => ({
          ...playhead,
          paused: false,
        }))
      },
      { signal: controller.signal },
    )

    vr.addEventListener(
      'pause',
      () => {
        setPlayhead(playhead => ({
          ...playhead,
          paused: true,
        }))
      },
      { signal: controller.signal },
    )

    vr.addEventListener(
      'seeking',
      () => {
        setPlayhead(playhead => ({
          ...playhead,
          seeking: true,
        }))
      },
      { signal: controller.signal },
    )

    vr.addEventListener(
      'seeked',
      () => {
        setPlayhead(playhead => ({
          ...playhead,
          seeking: false,
        }))
      },
      { signal: controller.signal },
    )

    vr.addEventListener(
      'durationchange',
      event => {
        setPlayhead(playhead => ({
          ...playhead,
          duration: event.target.duration,
        }))
      },
      { signal: controller.signal },
    )

    return () => {
      if (controller?.signal) {
        controller.abort()
      }
    }
  }, [videoRef, setPlayhead])

  console.log({ playhead })

  return (
    <video ref={ el => setVideo(el) } controls>
      <source src='https://archive.org/download/BigBuckBunny_124/Content/big_buck_bunny_720p_surround.mp4' />
    </video>
  )
}
