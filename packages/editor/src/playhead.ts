import { createModel, signal } from "@preact/signals-react";


// The Playhead is a tiny reactive model shared by BOTH caption-editor routes ---
// the incumbent Slate route and the Lexical spike --- so the two implementations
// are measured against identical video-sync machinery. Lifting it here keeps the
// comparison honest: neither editor gets a private, subtly-different playhead.
//
// `createModel` mints a class whose instances own the signals returned by the
// factory. `new Playhead()` therefore hands each caller its own {seek, timestamp}
// pair; below we export a single module-scoped singleton that both routes import.
//
//   - seek       --- write target: "move the video to this time" (click-to-seek).
//   - timestamp  --- read source: the video's current playback position.
export const Playhead = createModel(() => {
    const seek = signal<number>(0);
    const timestamp = signal<number>(0);

    return { seek, timestamp };
});


export const playhead = new Playhead();
