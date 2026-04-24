import { atom } from "recoil";

export const playerState = atom({
    default: {
        currentTime: 0,
        duration: 0,
        isPlaying: false,
        isSeeking: false,
        volume: 1,
    },
    key: "player:state",
});
