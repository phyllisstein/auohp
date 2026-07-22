import { createRouter } from "@tanstack/react-router";

import { routeTree } from "./routeTree.gen";
import client from "./apollo";

export function getRouter() {
    return createRouter({
        defaultPreload: "intent",
        routeTree,
        scrollRestoration: true,
        context: {
            apolloClient: client,
        },
    });
}

declare module "@tanstack/react-router" {
    interface Register {
        router: ReturnType<typeof getRouter>;
    }
}
