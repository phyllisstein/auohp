import { type ReactNode, useState } from "react";
import { ServerStyleSheet, StyleSheetManager } from "styled-components";

/**
 * Collects server-rendered styled-components rules and emits them into the
 * document head.
 *
 * The shape here is deliberately *not* the Next.js `useServerInsertedHTML`
 * registry that the styled-components docs lead with. TanStack Start has no
 * such hook, and `ServerStyleSheet.interleaveWithNodeStream` is unreachable
 * from here too: `renderRouterToStream` prefers `renderToReadableStream` when
 * react-dom exposes it (it does, under Vite's SSR runtime), so there is no Node
 * stream to interleave with --- and even the fallback branch pipes through
 * `transformPipeableStreamWithRouter` rather than handing us the raw pipeable.
 *
 * React 19's stylesheet hoisting replaces both. A `<style>` carrying `href` and
 * `precedence` is floated into `<head>` from wherever it renders, and is
 * de-duplicated by `href` on the client. That lets `Flush` sit *after*
 * `children` --- late enough that the sheet is actually full --- while the tag
 * itself still lands in the head. Ordering is the whole problem this solves:
 * `__root.tsx` renders its own `<head>`, and at the moment React walks that
 * element the sheet is still empty, so emitting there directly yields nothing.
 *
 * On the client this collapses to a passthrough: `ServerStyleSheet` is never
 * constructed, and styled-components injects through its own main sheet via
 * CSSOM as usual.
 */
export function StyledComponentsRegistry ({ children }: { children: ReactNode }) {
    // `import.meta.env.SSR` is statically replaced at build time, so the browser
    // bundle keeps only the passthrough branch and tree-shakes `ServerStyleSheet`
    // out entirely.
    if (!import.meta.env.SSR) {
        return <>{ children }</>;
    }

    return <ServerRegistry>{ children }</ServerRegistry>;
}

function ServerRegistry ({ children }: { children: ReactNode }) {
    // Lazy initial state so the sheet survives a re-render without being rebuilt.
    // x-ref: https://react.dev/reference/react/useState#avoiding-recreating-the-initial-state
    const [sheet] = useState(() => new ServerStyleSheet());

    return (
        <StyleSheetManager sheet={ sheet.instance }>
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
 * `getStyleElement` is used rather than v7's new `extractCSS` on purpose: it
 * keeps the `data-styled` / `/*!sc*​/` rehydration markers that the client
 * reads to rebuild group boundaries and skip re-injecting rules it already has.
 * `extractCSS` strips them by design (it targets shadow DOM and iframe
 * cloning), so styles would paint but then be duplicated on hydration.
 */
function Flush ({ sheet }: { sheet: ServerStyleSheet }) {
    return (
        <>
            { sheet.getStyleElement().map((style, index) => (
                <style
                    // eslint-disable-next-line react/no-danger
                    dangerouslySetInnerHTML={ style.props.dangerouslySetInnerHTML }
                    data-styled={ style.props["data-styled"] }
                    data-styled-version={ style.props["data-styled-version"] }
                    href={ `sc-${ index }` }
                    key={ style.key ?? index }
                    nonce={ style.props.nonce }
                    precedence="sc" />
            )) }
        </>
    );
}
