import {atom} from 'recoil'

export const playerState = atom({
  key: 'player:state',
  default: {
    currentTime: 0,
    duration: 0,
    isPlaying: false,
    isSeeking: false,
    volume: 1,
  },
})
