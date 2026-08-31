import { createRootRouteWithContext, HeadContent, Outlet, Scripts } from "@tanstack/react-router";
import { Provider as SpectrumProvider } from "@react-spectrum/s2/Provider";
import { ApolloProvider } from "@apollo/client/react";
import type { ApolloClientIntegration } from "@apollo/client-integration-tanstack-start";
import { useRouter } from "@tanstack/react-router";
import { ThemeProvider as StyledThemeProvider } from "styled-components";
import theme from "@/styles/theme";
import { Body } from "@/styles/global";
import { Button as SButton } from "@react-spectrum/s2";
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
        <ApolloProvider client={ apolloClient }>
            <SpectrumProvider
                background="base"
                colorScheme="dark"
                elementType="html"
                locale="en-US"
                router={{
                    navigate: (href, options) => {
                        if (typeof href === "string") return;
                        return router.navigate({ ...href, ...options });
                    },
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
                        <StyledThemeProvider theme={ theme }>
                            <Body />
                            <ErrorBoundary
                                fallback={
                                    ({ error, reset }) => {
                                        console.error("Error occurred: %o", error);
                                        return <div>Error: { error.message }</div>;
                                    }
                                }>
                                <Outlet />
                            </ErrorBoundary>
                        </StyledThemeProvider>
                    </main>
                    <Scripts />
                </body>
            </SpectrumProvider>
        </ApolloProvider>
    );
}

// Configure the type of the `href` and `routerOptions` props on all React Spectrum components.
declare module "@react-spectrum/s2/Provider" {
    interface RouterConfig {
        href: ToOptions;
        routerOptions: Omit<NavigateOptions, keyof ToOptions>;
    }
}
