import { createRootRoute, HeadContent, Outlet, Scripts } from "@tanstack/react-router";
import { Body } from "styles/global";

export const Route = createRootRoute({
    head: () => ({
        meta: [
            { charSet: "utf-8" },
            { content: "width=device-width, initial-scale=1", name: "viewport" },
        ],
    }),
    component: RootComponent,
});

function RootComponent() {
    return (
        <html lang="en-US">
            <head>
                <HeadContent />
            </head>
            <body>
                <Body />
                <sp-theme color="light" scale="medium" system="spectrum">
                    <main className="spectrum-Body spectrum-Body--sizeXL">
                        <Outlet />
                    </main>
                </sp-theme>
                <Scripts />
            </body>
        </html>
    );
}
