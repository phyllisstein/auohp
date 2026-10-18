import { type ReactNode, useState } from "react";
import { ServerStyleSheet, StyleSheetManager } from "styled-components";

/**
 * Collects server-rendered styled-components rules and emits them into the
 * document.
 *
 * The shape here is deliberately *not* the Next.js `useServerInsertedHTML`
 * registry that the styled-components docs lead with. TanStack Start has no
 * such hook, and `ServerStyleSheet.interleaveWithNodeStream` is unreachable
 * from here too: `renderRouterToStream` prefers `renderToReadableStream` when
 * react-dom exposes it (it does, under Vite's SSR runtime), so there is no Node
 * stream to interleave with --- and even the fallback branch pipes through
 * `transformPipeableStreamWithRouter` rather than handing us the raw pipeable.
 *
 * Two constraints govern everything below, and they pull against each other.
 *
 * The first is `useId`. React derives an id from the component's *path* through
 * the element tree --- `pushTreeContext` packs "child `index` of
 * `totalChildren`" into a bit-field, one layer per nesting level, and `useId`
 * base-32 encodes the result. So the id is positional, and `totalChildren`
 * participates: adding a sibling *anywhere on an ancestor path* renumbers every
 * id beneath it. This component wraps the whole app, which makes it the highest
 * possible perturbation point. If the server renders `children` plus N `<style>`
 * siblings and the client renders `children` alone, every React Aria `id`,
 * `aria-labelledby` and `aria-describedby` below diverges --- an accessibility
 * break, not merely a console warning.
 *
 * That is why there is no `import.meta.env.SSR` *structural* branch here. The
 * branch is on the sheet's *value*, never on the element shape: `<Flush />` is
 * rendered unconditionally, so it occupies its slot on both sides and renders
 * nothing on the client. Note that what the tree context counts is the presence
 * of the *element*, not what its component returns --- so the invariant to hold
 * is that `<Flush />` stays unconditional. Reintroducing an early return above
 * it, or wrapping it in a condition, is what would drift the ids again.
 *
 * The second is rehydration. styled-components adopts the server sheet by
 * querying `style[data-styled][data-styled-version="<exact version>"]`, parsing
 * the `/*!sc*\/` group markers, and then *removing* the tag. React 19's
 * stylesheet hoisting is incompatible with that: a `<style>` carrying
 * `precedence` is floated into `<head>` with its attributes stripped, so the
 * query never matches and every rule is injected a second time on the client.
 * The tags below therefore carry no `precedence` and no `href`. A `<style>` in
 * the body is applied document-wide by CSSOM regardless of position, so the
 * markers are worth more than the hoist.
 *
 * A note on the version pin, because it is load-bearing for the first
 * constraint. This package is held at styled-components 6 deliberately. In the
 * v7 prereleases the browser build wraps every styled component's output in a
 * `Fragment` --- `[<sc-inject/>, element]` --- to host the `useInsertionEffect`
 * that injects its rules, while the server build returns the bare element. That
 * is the same shape branch this file avoids, but inside the library and
 * unreachable from userland: it keys off `styleSheet.server`, which is set from
 * `isServer` and cannot be true on the client without disabling injection. The
 * result is one extra tree-context level on the client under *every* styled
 * component, so any `useId` below one drifts --- React Aria's, in practice.
 * Verify the branch is gone before upgrading.
 */
export function StyledComponentsRegistry ({ children }: { children: ReactNode }) {
    // Lazy initial state so the sheet survives a re-render without being
    // rebuilt. On the client the initializer yields `null` --- the *value*
    // differs across environments, the element tree does not.
    // x-ref: https://react.dev/reference/react/useState#avoiding-recreating-the-initial-state
    const [sheet] = useState(() => (import.meta.env.SSR ? new ServerStyleSheet() : null));

    return (
        <StyleSheetManager sheet={ sheet?.instance }>
            { children }
            <Flush sheet={ sheet } />
        </StyleSheetManager>
    );
}

/**
 * Renders the collected CSS. Kept as its own component so React only evaluates
 * it after `children` has been walked --- reading the sheet inline in the
 * parent would sample it before any styled component had contributed a rule.
 *
 * `getStyleElement` keeps the `data-styled` / `/*!sc*\/` rehydration markers
 * that the client reads to rebuild group boundaries and skip re-injecting rules
 * it already has. `getStyleTags` would serialise to a string we would then have
 * to feed through `dangerouslySetInnerHTML` ourselves, and `_emitSheetCSS` is
 * private --- the element form is the supported seam.
 */
function Flush ({ sheet }: { sheet: ServerStyleSheet | null }) {
    // On the client there is no collected sheet, so there is nothing to render.
    // `null` is safe here, and the reason is worth being precise about: what
    // `pushTreeContext` counts is the *element* in the parent's children array,
    // not whatever that element's component returns. `StyledComponentsRegistry`
    // renders `<Flush />` unconditionally on both sides, so it holds its slot
    // either way and the ids below are unaffected. `<></>` would work too.
    //
    // The invariant to protect is therefore the one above --- that `<Flush />`
    // is *present* in both trees --- not anything about this return value. It
    // was the old `import.meta.env.SSR` early-return, which omitted the element
    // entirely on the client, that shifted every id.
    if (!sheet) {
        return null;
    }

    return (
        <>
            { sheet.getStyleElement().map((style, index) => (
                <style
                    // eslint-disable-next-line react/no-danger
                    dangerouslySetInnerHTML={ style.props.dangerouslySetInnerHTML }
                    data-styled={ style.props["data-styled"] }
                    data-styled-version={ style.props["data-styled-version"] }
                    key={ style.key ?? index }
                    nonce={ style.props.nonce } />
            )) }
        </>
    );
}
