import { defineExtension } from "lexical";
import { namedSignals, type Signal } from "@lexical/extension";
import { ReactExtension } from "@lexical/react/ReactExtension";
import { useExtensionSignalValue } from "@lexical/react/useExtensionSignalValue";
import type { JSX } from "react";

// -----------------------------------------------------------------------------
// LatencyExtension --- litmus test 1 instrumentation, and the canonical shape for
// "an extension that also has a face".
//
// `build` is the hook for producing values other code consumes: it returns an
// `output` object, here a `stats` signal plus a `Component`. An extension whose
// output carries a `Component` satisfies `OutputComponentExtension`, which is
// exactly what `useExtensionComponent` consumes --- so React reaches the meter by
// asking the extension for it, rather than the meter reaching into React context
// for the editor. The dependency arrow reverses.
//
// Note also that `lastKeystrokeAt` is now a closure local rather than a module
// global. Under the old plugin, two editors on one page shared that variable and
// silently corrupted each other's measurements; per-editor `register` scope fixes
// that for free.
// -----------------------------------------------------------------------------
export interface LatencyStats {
    count: number;
    lastMs: number;
    maxMs: number;
}

export interface LatencyOutput {
    stats: Signal<LatencyStats>;
    Component: () => JSX.Element;
}

export const LatencyExtension = /* @__PURE__ */ defineExtension({
    build: (): LatencyOutput => ({
        ...namedSignals({ stats: { count: 0, lastMs: 0, maxMs: 0 } as LatencyStats }),
        Component: LatencyMeter,
    }),
    dependencies: [ReactExtension],
    name: "@auohp/latency",
    register (editor, _config, state) {
        const { stats } = state.getOutput();

        // Lexical is UNCONTROLLED: the ContentEditable owns its DOM and React does
        // not re-render per keystroke (the exact opposite of Slate's controlled
        // <Editable/>). To make that observable we stamp `performance.now()` on
        // each `beforeinput` and measure the gap to the resulting EditorState
        // update.
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
            // Writing `.value` notifies subscribers; the React meter re-renders,
            // and nothing else does --- which is the property under test.
            const { count, maxMs } = stats.peek();
            stats.value = {
                count: count + 1,
                lastMs: delta,
                maxMs: Math.max(maxMs, delta),
            };
        });

        return () => {
            unregisterRoot();
            unregisterUpdate();
        };
    },
});

function LatencyMeter (): JSX.Element {
    // `useExtensionSignalValue` bridges the signal to React via
    // useSyncExternalStore --- no editor, no context, no useEffect.
    const stats = useExtensionSignalValue(LatencyExtension, "stats");

    return (
        <div style={{ fontFamily: "monospace", fontSize: "0.8rem", opacity: 0.8 }}>
            edits: { stats.count } | last: { stats.lastMs.toFixed(2) }ms | max: { stats.maxMs.toFixed(2) }ms
        </div>
    );
}
