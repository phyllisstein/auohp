/**
 * Widens the type of `location.state` --- the payload carried through
 * `router.navigate({ state })` and read back as `useRouterState().location.state`.
 *
 * `@tanstack/history` ships `HistoryState` as an empty interface for exactly
 * this purpose: it is an open extension point, and `ParsedHistoryState` is an
 * alias that intersects it with the router's own bookkeeping keys
 * (`key`, `__TSR_key`, `__TSR_index`). Merging a field here therefore reaches
 * every `location.state` read in the app without touching a call site.
 *
 * The augmentation must name `@tanstack/history`, the package that *declares*
 * the interface. `@tanstack/react-router` re-exports it with
 * `export type { HistoryState } from "@tanstack/history"`, and augmenting a
 * re-export declares a new, unrelated interface in that module rather than
 * merging into the original --- silently, with the original errors intact.
 *
 * Every key must be optional. History state is whatever a previous navigation
 * happened to put there: a fresh visit, a hard reload, or a back-navigation to
 * an entry written before the key existed all yield `undefined`. Typing a key
 * as required would let `strictNullChecks` wave through reads that are empty at
 * runtime, which is the failure this file exists to prevent.
 */
declare module "@tanstack/history" {
    interface HistoryState {
        /**
         * Seeds the search box on `/search`, so a result page can be linked to,
         * reloaded, or reached with the back button and still show the query
         * that produced it. Written by the search route after a successful
         * query; read on mount to restore the field.
         */
        query?: string;
    }
}

// A file with no top-level import or export is a *global* script, and a
// `declare module` inside one is read as an ambient wildcard declaration
// rather than an augmentation of the real module. This empty export marks the
// file as a module so the merge above targets the package it names.
export {};
