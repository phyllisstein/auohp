import { defineExtension, type Signal } from "lexical";
import { namedSignals } from "@lexical/extension";
import { ReactExtension, useExtensionComponent } from "@lexical/react/ReactExtension";
import { configExtension } from "lexical";
import { useEffect, useState, type JSX } from "react";
import { ProgressCircle } from "@react-spectrum/s2/ProgressCircle";

export interface LatencyStats {
    count: number;
    lastMs: number;
    maxMs: number;
}

export interface LatencyOutput {
    stats: Signal<LatencyStats>;
    Component: () => JSX.Element;
}

function LatencyMeter (): JSX.Element {
    const stats = { count: 0, lastMs: 0, maxMs: 0 };

    return (
        <div style={{ fontSize: "0.875rem", color: "#666", padding: "0.25rem 0.5rem" }}>
            <span>{stats.count} edits</span>
            {stats.lastMs > 0 && <span> {stats.lastMs}ms</span>}
            {stats.maxMs > 0 && <span> (max {stats.maxMs}ms)</span>}
        </div>
    );
}

export const LatencyExtension = /* @__PURE__ */ defineExtension({
    build: (): LatencyOutput => ({
        ...namedSignals({ stats: { count: 0, lastMs: 0, maxMs: 0 } as LatencyStats }),
        Component: LatencyMeter,
    }),
    dependencies: [ReactExtension],
    name: "@auohp/latency",
    register (editor, _config, state) {
        const { stats } = state.getOutput();
        let lastKeystrokeAt = 0;

        return editor.registerUpdateListener(({ updateTags }) => {
            if (updateTags.has("history-merge")) {
                return;
            }

            const now = performance.now();
            const elapsed = now - lastKeystrokeAt;

            if (lastKeystrokeAt > 0) {
                stats.value = {
                    count: stats.peek().count + 1,
                    lastMs: Math.round(elapsed),
                    maxMs: Math.max(stats.peek().maxMs, Math.round(elapsed)),
                };
            }

            lastKeystrokeAt = now;
        });
    },
});
