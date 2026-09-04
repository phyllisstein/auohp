// -----------------------------------------------------------------------------
// The `namedSignals` -> Svelte `$state` bridge.
//
// In React the editor's own state channel is @lexical/extension's
// `namedSignals({ stats })`, read in a component with
// `useExtensionSignalValue(SomeExtension, "stats")` --- a hook per signal per
// component, and a Babel transform (@preact/signals-react-transform) to make
// bare `.value` reads reactive at all.
//
// Svelte's equivalent is a module-level `$state` object in a `.svelte.ts` file.
// The `.svelte.ts` extension is the whole ceremony: it tells the compiler to
// treat runes as legal outside a component, so a plain exported object becomes
// a reactive singleton. A component reads `tagPalette.color` --- no hook, no
// import of the extension, no transform config --- and any template or
// `$derived` that touched it re-runs.
//
// Two shapes are shown:
//   1. `tagPalette` --- a Svelte-native reactive singleton, for state the
//      extension owns outright.
//   2. `bridgeSignal` --- a genuine @lexical/extension `Signal` (the same type
//      `namedSignals` produces) adapted into a `$state` box. This is what you
//      would use for extensions whose signals are already load-bearing in
//      framework-agnostic `register()` code and must stay Signals.
// -----------------------------------------------------------------------------

import type { Signal } from "@lexical/extension";

/** Case 1: state that only the UI cares about. Pure Svelte, zero adapter. */
export const tagPalette = $state({ color: "#b3366b", label: "person" });

export const PALETTE = [
    { color: "#b3366b", label: "person" },
    { color: "#2a7f62", label: "org" },
    { color: "#2f5d9e", label: "place" },
    { color: "#8a5a1f", label: "event" },
];

export function cyclePalette() {
    const index = PALETTE.findIndex(entry => entry.color === tagPalette.color);
    const next = PALETTE[(index + 1) % PALETTE.length];
    tagPalette.color = next.color;
    tagPalette.label = next.label;
}

/**
 * Case 2: adapt a real `namedSignals` Signal into a rune.
 *
 * `$state` holds the latest value; a `$effect.root` owns the subscription so it
 * is torn down explicitly rather than tied to a component instance. The getter
 * is returned rather than the box, so callers write `stats()` and get
 * fine-grained reactivity without being able to write back into the mirror.
 *
 * The asymmetry worth naming: React needs the hook because a component only
 * re-renders when something tells it to. Svelte's rune is a signal already, so
 * this adapter is literally signal->signal plumbing --- 10 lines, once, per
 * signal type, and reusable.
 */
export function bridgeSignal<T>(signal: Signal<T>): { value: T; dispose: () => void } {
    const box = $state({ value: signal.peek() });
    const unsubscribe = signal.subscribe((next: T) => {
        box.value = next;
    });
    return {
        get value() {
            return box.value;
        },
        set value(next: T) {
            signal.value = next;
        },
        dispose: unsubscribe,
    };
}
