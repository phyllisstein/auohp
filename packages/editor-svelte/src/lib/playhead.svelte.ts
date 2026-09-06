// Port of packages/editor/src/playhead.ts.
//
// Factory only, no default instance: the route creates one playhead per
// interview and hands it to the extensions as config. Unlike `searchQuery`
// (search.svelte.ts), which stays a deliberate shared singleton.

export interface Playhead {
    // Write target: "move the video to this time" (click-to-seek).
    seek: number;
    // Read source: the video's current playback position.
    timestamp: number;
}

export function createPlayhead (): Playhead {
    const playhead = $state({
        seek: 0,
        timestamp: 0,
    });

    return playhead;
}
