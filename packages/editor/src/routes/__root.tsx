import { createRootRoute, HeadContent, Outlet, Scripts } from "@tanstack/react-router";
import { Body } from "styles/global";
import { Provider } from "@react-spectrum/s2/Provider";

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
        <Provider background="layer-1" colorScheme="light" elementType="html" locale="en-US">
            <head>
                <HeadContent />
            </head>
            <body>
                <main>
                    <Outlet />
                </main>
                <Scripts />
            </body>
        </Provider>
    );
}
