import { createFileRoute } from "@tanstack/react-router";
import { useState, useEffect } from "react";
import { Badge, Text } from "@react-spectrum/s2/Badge";
import ClockPending from "@react-spectrum/s2/icons/ClockPending";

export const Route = createFileRoute("/transcript/create")({
    component: NewTranscriptPage,
});

function NewTranscriptPage () {
    const fetchDesktopHealth = async () => {
        const response = await fetch("http://localhost:8705/health");
        const data = await response.json();
        console.log(data);
        return data;
    };

    const [health, setHealth] = useState(() => null);

    useEffect(() => {
        async function fetchData () {
            const json = await fetchDesktopHealth();
            setHealth(json);
            console.log("Fetched desktop health:\t", json);
        }
        fetchData();
    }, []);

    return (
        <section style={{ width: "min-content" }}>
            {
                health?.status === "ok"
                    ? (
                        <Badge size="XL" variant="positive">
                            <Text>Healthy</Text>
                        </Badge>
                    )
                    : (
                        <Badge size="XL" fillStyle="subtle" variant="neutral">
                            <ClockPending />
                            <Text>Loading</Text>
                        </Badge>
                    )
            }
        </section>
    );
}
