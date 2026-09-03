import { createFileRoute, Outlet } from "@tanstack/react-router";

export const Route = createFileRoute("/transcript")({
    component: TranscriptRoutePage,
});
function TranscriptRoutePage () {
    return <Outlet />;
}
