import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/")({
    component: Page,
    ssr: false,
    beforeLoad: () => {
        import("@spectrum-web-components/theme/sp-theme.js");
        import("@spectrum-web-components/theme/src/themes.js");
        import("@spectrum-web-components/theme/theme-light.js");
        import("@spectrum-web-components/theme/scale-large.js");
        import("@spectrum-web-components/button/sp-button.js");
        import("@spectrum-web-components/badge/sp-badge.js");
    },
});


function Page() {
    return (
        <>
            <sp-button onClick={ () => console.log("Button clicked") }>Try me</sp-button>
        </>
    );
}
