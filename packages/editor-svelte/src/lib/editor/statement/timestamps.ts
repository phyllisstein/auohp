// No shipped browser has Temporal yet (TC39 Stage 3) -- explicit import
// rather than the global shim so the dependency is visible here instead of
// a side-effecting import elsewhere.
import { Temporal } from "temporal-polyfill";

export const formatTimestamp = (timestamp: number) =>
    Temporal.Duration.from({ seconds: Math.round(timestamp) })
        .round({
            largestUnit: "hours",
            smallestUnit: "seconds",
        })
        .toLocaleString("en-US", {
            style: "digital",
            hoursDisplay: "auto",
            hours: "numeric",
        });
