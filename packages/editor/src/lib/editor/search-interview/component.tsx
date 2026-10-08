import { type NodeKey } from "lexical";
import { useCallback, useRef, useEffect, type JSX } from "react";
import styled from "styled-components";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { useExtensionDependency, useExtensionSignalValue } from "@lexical/react/useExtensionSignalValue";
import { Button } from "@react-spectrum/s2/Button";
import { TextField } from "@react-spectrum/s2/TextField";
import { ActionButton, Text } from "@react-spectrum/s2/ActionButton";
import { ActionButtonGroup } from "@react-spectrum/s2/ActionButtonGroup";
import SearchIcon from "@react-spectrum/s2/icons/Search";
import ChevronUpIcon from "@react-spectrum/s2/icons/ChevronUp";
import ChevronDownIcon from "@react-spectrum/s2/icons/ChevronDown";
import { ProgressCircle } from "@react-spectrum/s2/ProgressCircle";
import { SearchInterviewExtension } from "./extension";
import { SearchResult } from "./node";

const SearchContainer = styled.div`
    position: fixed;
    z-index: 1000;
    right: 0;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    align-items: flex-end;
    justify-content: center;
    width: 100%;
    height: max-content;
    min-height: max-content;
    padding: 1rem;
`;

const SearchFieldContainer = styled.div`
    width: 50%;
`;

const ButtonGroupContainer = styled.div`
    display: flex;
    align-items: center;
    justify-content: flex-end;
    width: max-content;
`;

export function SearchBar (): JSX.Element {
    const { query, focusedResult, resultKeys, replacement } = useExtensionDependency(SearchInterviewExtension).output;
    const queryValue = useExtensionSignalValue(SearchInterviewExtension, "query");
    const loading = useExtensionSignalValue(SearchInterviewExtension, "loading");
    const resultCount = useExtensionSignalValue(SearchInterviewExtension, "resultCount");
    const replacementValue = useExtensionSignalValue(SearchInterviewExtension, "replacement");
    const [editor] = useLexicalComposerContext();

    const spinner = (
        <ProgressCircle
            aria-label="Loading…"
            value={80}
            isIndeterminate
            size="S"
            staticColor="white" />
    );

    const focusNext = useCallback(() => {
        if (resultCount === 0) {
            return;
        }
        focusedResult.value = ((focusedResult.peek() ?? -1) + 1) % resultCount;
    }, [focusedResult, resultCount]);

    const focusPrevious = useCallback(() => {
        if (resultCount === 0) {
            return;
        }
        focusedResult.value = ((focusedResult.peek() ?? 0) - 1 + resultCount) % resultCount;
    }, [focusedResult, resultCount]);

    return (
        <SearchContainer>
            <SearchFieldContainer>
                <TextField
                    aria-label="Search transcript"
                    type="search"
                    enterKeyHint="search"
                    inputMode="search"
                    prefix={loading ? spinner : <SearchIcon />}
                    size="M"
                    value={queryValue ?? ""}
                    onChange={value =>
                        query.value = value.length > 0 ? value : null} />
            </SearchFieldContainer>
            <ButtonGroupContainer>
                <ActionButtonGroup isDisabled={queryValue === null || loading || !resultCount}>
                    <ActionButton onPress={focusPrevious}>
                        <ChevronUpIcon />
                        <Text>Previous</Text>
                    </ActionButton>
                    <ActionButton onPress={focusNext}>
                        <ChevronDownIcon />
                        <Text>Next</Text>
                    </ActionButton>
                </ActionButtonGroup>
            </ButtonGroupContainer>
        </SearchContainer>
    );
}
