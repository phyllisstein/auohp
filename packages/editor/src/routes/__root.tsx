import { createRootRouteWithContext, HeadContent, Outlet, Scripts } from "@tanstack/react-router";
import { Provider as SpectrumProvider } from "@react-spectrum/s2/Provider";
import { Body } from "@/styles/global";
import { ApolloProvider } from "@apollo/client/react";
import { type ApolloClient } from "@apollo/client";


export interface RouteContext {
    apolloClient: ApolloClient;
}


export const Route = createRootRouteWithContext<RouteContext>()({
    component: RootComponent,
    notFoundComponent: () => <div>Not found</div>,
    head: () => ({
        meta: [
            { charSet: "utf-8" },
            { content: "width=device-width, initial-scale=1", name: "viewport" },
        ],
    }),
});

function RootComponent() {
    const { apolloClient } = Route.useRouteContext();

    return (
        <ApolloProvider client={ apolloClient }>
            <SpectrumProvider background="layer-1" colorScheme="light" elementType="html" locale="en-US">
                <head>
                    <HeadContent />
                </head>
                <body>
                    <Body />
                    <main>
                        <Outlet />
                    </main>
                    <Scripts />
                </body>
            </SpectrumProvider>
        </ApolloProvider>
    );
}
