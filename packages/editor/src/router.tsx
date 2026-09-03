import { createRouter } from "@tanstack/react-router";

import { routeTree } from "./routeTree.gen";

import {
    routerWithApolloClient,
    ApolloClient,
    InMemoryCache,
} from "@apollo/client-integration-tanstack-start";
import { HttpLink } from "@apollo/client";


export function getRouter () {
    const apolloClient = new ApolloClient({
        link: new HttpLink({
            uri: import.meta.env.VITE_AUOHP_API_URI?.replace(/\/+$/, "") + "/graphql",
        }),
        cache: new InMemoryCache(),
    });

    const router = createRouter({
        defaultPreload: false,
        routeTree,
        context: {
            ...routerWithApolloClient.defaultContext,
        },
    });

    return routerWithApolloClient(router, apolloClient);
}

declare module "@tanstack/react-router" {
    interface Register {
        router: ReturnType<typeof getRouter>;
    }
}
