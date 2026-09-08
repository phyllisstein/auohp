import { defineExtension } from "lexical";

// Measures the gap between a keystroke and the resulting EditorState update,
// to make Lexical's UNCONTROLLED model observable: the ContentEditable owns
// its DOM and nothing re-renders per keystroke. Chrome (a meter component)
// is out of scope here -- this extension exposes stats only; a Svelte
// component reads them directly with $derived, no signal bridge needed.
//
// Output is a $state object rather than a namedSignals bundle. Every field
// changes together on every update and nothing needs per-field granularity,
// so there's no split to make the way persistence's delay/destroyDelay have --
// build() creating $state means this file must be named *.svelte.ts, which
// vite-plugin-svelte compiles as a rune module regardless of extension.
export interface LatencyStats {
    count: number;
    lastMs: number;
    maxMs: number;
}

export const LatencyExtension = /* @__PURE__ */ defineExtension({
    name: "@auohp/latency",
    build: () => {
        // $state() must be a variable declaration initializer -- returning
        // its call expression directly doesn't compile.
        const stats = $state<LatencyStats>({ count: 0, lastMs: 0, maxMs: 0 });
        return stats;
    },
    register (editor, _config, state) {
        const stats = state.getOutput();

        // lastKeystrokeAt is a closure local, not a module global: one meter
        // per editor instance, same reasoning as the playhead per-instance
        // decision.
        let lastKeystrokeAt = 0;
        const stamp = () => {
            lastKeystrokeAt = performance.now();
        };

        const unregisterRoot = editor.registerRootListener((rootEl, prevRootEl) => {
            prevRootEl?.removeEventListener("beforeinput", stamp);
            rootEl?.addEventListener("beforeinput", stamp);
        });

        const unregisterUpdate = editor.registerUpdateListener(() => {
            if (lastKeystrokeAt === 0) {
                return;
            }
            const delta = performance.now() - lastKeystrokeAt;
            lastKeystrokeAt = 0;
            stats.count += 1;
            stats.lastMs = delta;
            stats.maxMs = Math.max(stats.maxMs, delta);
        });

        return () => {
            unregisterRoot();
            unregisterUpdate();
        };
    },
});
