import { atom } from 'recoil'

export const playhead = atom({
  key: 'player:playhead',
  default: {
    currentTime: 0,
    duration: 0,
    paused: true,
    seeking: false,
  },
})
