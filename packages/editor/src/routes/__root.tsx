import { createRootRoute, HeadContent, Outlet, Scripts } from "@tanstack/react-router";
import { Provider as SpectrumProvider } from "@react-spectrum/s2/Provider";
import { ApolloClient, HttpLink, InMemoryCache } from "@apollo/client";
import { ApolloProvider } from "@apollo/client/react";

const client = new ApolloClient({
    link: new HttpLink({
        uri: import.meta.env.VITE_GRAPHQL_URI,
    }),
    cache: new InMemoryCache(),
});

export const Route = createRootRoute({
    head: () => ({
        meta: [
            { charSet: "utf-8" },
            { content: "width=device-width, initial-scale=1", name: "viewport" },
        ],
    }),
    component: RootComponent,
    notFoundComponent: () => <div>Not found</div>,
});

function RootComponent() {
    return (
        <SpectrumProvider background="layer-1" colorScheme="light" elementType="html" locale="en-US">
            <ApolloProvider client={ client }>
                <head>
                    <HeadContent />
                </head>
                <body>
                    <main>
                        <Outlet />
                    </main>
                    <Scripts />
                </body>
            </ApolloProvider>
        </SpectrumProvider>
    );
}
