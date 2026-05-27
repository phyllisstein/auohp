import { createRootRoute, HeadContent, Outlet, ScrollRestoration, Scripts } from "@tanstack/react-router";
import { useLayoutEffect } from "react";

import { StyledComponentsRegistry } from "styles/global";

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
    useLayoutEffect(() => {
        import("@spectrum-web-components/theme/sp-theme.js");
        import("@spectrum-web-components/theme/src/themes.js");
        import("@spectrum-web-components/theme/theme-light.js");
        import("@spectrum-web-components/theme/scale-large.js");
        import("@spectrum-web-components/button/sp-button.js");
        import("@spectrum-web-components/badge/sp-badge.js");
    }, []);
    return (
        <StyledComponentsRegistry>
            <html className="spectrum spectrum--large spectrum--light" lang="en">
                <head>
                    <HeadContent />
                </head>
                <body>

                    <sp-theme color="light" scale="large" system="spectrum">
                        <main className="spectrum-Body spectrum-Body--sizeXL">
                            <Outlet />
                        </main>
                    </sp-theme>
                    <ScrollRestoration />
                    <Scripts />
                </body>
            </html>
        </StyledComponentsRegistry>
    );
}
