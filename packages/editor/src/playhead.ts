import { createModel, signal } from "@preact/signals-react";


// The Playhead couples one <video> to one editor: the route that owns the video
// creates an instance per interview and hands it to the editor as
// StatementExtension config.
//
// There is deliberately no module-level instance. A shared singleton was an
// artifact of measuring the Slate and Lexical routes against identical
// video-sync machinery; with one video per interview route, ambient shared
// state only lets two editors seek each other's video.
//
// `createModel` mints a class whose instances own the signals returned by the
// factory. The class stays private to this module: `createPlayhead()` is the only
// way to get one, which matches editor-svelte's `playhead.svelte.ts`.
//
//   - seek       --- write target: "move the video to this time" (click-to-seek).
//   - timestamp  --- read source: the video's current playback position.
const PlayheadModel = createModel(() => {
    const seek = signal<number>(0);
    const timestamp = signal<number>(0);

    return { seek, timestamp };
});

export type Playhead = InstanceType<typeof PlayheadModel>;

export function createPlayhead (): Playhead {
    return new PlayheadModel();
}
