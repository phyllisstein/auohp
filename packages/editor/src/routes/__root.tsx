// Must precede every other stylesheet import: it fixes cascade-layer order
// before any layer-using CSS (Spectrum's `page.css`, component chunks) is parsed.
import "@/styles/global/layers.css";
import "@react-spectrum/s2/page.css";
import { createRootRouteWithContext, HeadContent, Outlet, Scripts, type ToOptions, type NavigateOptions } from "@tanstack/react-router";
import { Provider as SpectrumProvider } from "@react-spectrum/s2/Provider";
import { ApolloProvider } from "@apollo/client/react";
import type { ApolloClientIntegration } from "@apollo/client-integration-tanstack-start";
import { useRouter } from "@tanstack/react-router";
import { ThemeProvider as StyledThemeProvider } from "styled-components";
import theme from "@/styles/theme";
import { Body, StyledComponentsRegistry } from "@/styles/global";
import { AdobeClean } from "@/styles/assets/fonts";
import { ErrorBoundary } from "@suspensive/react";


export const Route = createRootRouteWithContext<ApolloClientIntegration.RouterContext>()({
    component: RootComponent,
    notFoundComponent: () => <div>Not found</div>,
    head: () => ({
        meta: [
            { charSet: "utf-8" },
            { content: "width=device-width, initial-scale=1", name: "viewport" },
        ],
    }),
});

function RootComponent () {
    const { apolloClient } = Route.useRouteContext();
    const router = useRouter();

    return (
        <StyledComponentsRegistry>
            <StyledThemeProvider theme={ theme }>
                <ApolloProvider client={ apolloClient }>
                    <AdobeClean />
                    <Body />
                    <SpectrumProvider
                        background="layer-1"
                        colorScheme="dark"
                        elementType="html"
                        locale="en-US"
                        router={{
                        // S2's `Router` type declares `navigate(path: string, ...)`
                        // outright --- unlike `useHref`, its first parameter is not
                        // wired to the augmentable `Href`, so the `RouterConfig`
                        // augmentation below never reaches it. `href` really is a
                        // string here, so navigate by `to` rather than spreading it.
                            navigate: (href, options) => router.navigate({ to: href, ...options }),
                            useHref: href => {
                                if (typeof href === "string") return href;
                                return router.buildLocation(href).href;
                            },
                        }}>
                        <head>
                            <HeadContent />
                        </head>
                        <body>
                            <main>
                                <ErrorBoundary
                                    fallback={
                                        ({ error, reset }) => {
                                            console.error("Error occurred: %o", error);
                                            return (
                                                <div>
                                                    <div>Error: { error.message }</div>
                                                    <button onClick={ reset }>Retry</button>
                                                </div>
                                            );
                                        }
                                    }>
                                    <Outlet />
                                </ErrorBoundary>
                            </main>
                            <Scripts />
                        </body>
                    </SpectrumProvider>
                </ApolloProvider>
            </StyledThemeProvider>
        </StyledComponentsRegistry>
    );
}

// Configure the type of the `href` and `routerOptions` props on all React Spectrum components.
declare module "@react-spectrum/s2/Provider" {
    interface RouterConfig {
        href: ToOptions;
        routerOptions: Omit<NavigateOptions, keyof ToOptions>;
    }
}
