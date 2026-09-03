import { usePortal } from "./use-portal";
import { createPortal } from "react-dom";
import type { ReactPortal, PropsWithChildren } from "react";
import { ResultsContainer } from "./results-styles";

export interface ResultsProps {
    bottom?: number;
    height?: number;
    left?: number;
    right?: number;
    top?: number;
    width?: number;
}

export function Results ({
    bottom = 0,
    children,
    height = 0,
    left = 0,
    right = 0,
    top = 0,
    width = 0,
}: PropsWithChildren<ResultsProps>): ReactPortal {
    const portal = usePortal();

    if (!portal) {
        return null;
    }

    top = bottom ?? height + top;

    return createPortal(
        <ResultsContainer style={{ left, top, width }}>
            { children }
        </ResultsContainer>,
        portal,
    );
}
