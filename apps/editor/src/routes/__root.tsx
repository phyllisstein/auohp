import {
    HeadContent,
    Scripts,
    createRootRoute,
} from "@tanstack/react-router";
import { TanStackRouterDevtoolsPanel } from "@tanstack/react-router-devtools";
import { TanStackDevtools } from "@tanstack/react-devtools";
import { ReactQueryDevtoolsPanel } from "@tanstack/react-query-devtools";
import { RouteNavigationPanel } from "#/devtools";
import { ErrorBoundary } from "@suspensive/react";

export const Route = createRootRoute({
    shellComponent: RootDocument,
    notFoundComponent: () => <div>Not found</div>,
    head: () => ({
        meta: [
            { charSet: "utf-8" },
            { content: "width=device-width, initial-scale=1", name: "viewport" },
        ],
    }),
});

function RootDocument ({ children }: { children: React.ReactNode }) {
    return (
        <html lang="en">
            <head>
                <HeadContent />
            </head>
            <body>
                <TanStackDevtools
                    config={{
                        position: "bottom-right",
                    }}
                    plugins={ [
                        {
                            name: "Router",
                            render: <TanStackRouterDevtoolsPanel />,
                        },
                        {
                            name: "Query",
                            render: <ReactQueryDevtoolsPanel />,
                        },
                        {
                            name: "Navigation",
                            render: <RouteNavigationPanel />,
                        },
                    ] } />
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
                        { children }
                    </ErrorBoundary>
                </main>
                <Scripts />
            </body>
        </html>
    );
}
