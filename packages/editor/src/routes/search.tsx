import { ProgressCircle } from "@react-spectrum/s2/ProgressCircle";
import { TextField } from "@react-spectrum/s2/TextField";
import SearchIcon from "@react-spectrum/s2/icons/Search";
import { createFileRoute } from "@tanstack/react-router";
import { style } from "@react-spectrum/s2/style" with { type: "macro" };
import styled from "styled-components";

export const Route = createFileRoute("/search")({
    component: SearchPage,
});

const ResultsContainer = styled.div`
    display: flex;
`;

function SearchPage () {
    const spinner = (
        <ProgressCircle
            aria-label="Loading…"
            value={ 80 }
            isIndeterminate
            size="S"
            staticColor="white" />
    );

    return (
        <div>
            <div className={ style({ backgroundColor: "layer-2", height: "full", padding: "text-to-control", margin: "text-to-control", borderRadius: "sm" }) }>
                <TextField
                    aria-label="Search transcript"
                    type="search"
                    enterKeyHint="search"
                    inputMode="search"
                    prefix={ false ? spinner : <SearchIcon /> }
                    size="M" />
            </div>
            <ResultsContainer className={ style({ backgroundColor: "layer-2", height: "full", padding: "text-to-control", margin: "text-to-control", borderRadius: "sm" }) }>
                <div className={ style({ width: "max", height: "max" }) }>
                    <h3>Search results</h3>
                </div>
            </ResultsContainer>
        </div>
    );
}
