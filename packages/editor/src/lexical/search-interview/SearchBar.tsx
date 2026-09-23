import { $getNodeByKey } from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { useExtensionDependency } from "@lexical/react/useExtensionComponent";
import { useExtensionSignalValue } from "@lexical/react/useExtensionSignalValue";
import { useCallback, type JSX } from "react";
import styled from "styled-components";
import { ActionButton, Text } from "@react-spectrum/s2/ActionButton";
import { ActionButtonGroup } from "@react-spectrum/s2/ActionButtonGroup";
import { ProgressCircle } from "@react-spectrum/s2/ProgressCircle";
import { TextField } from "@react-spectrum/s2/TextField";
import ChevronDownIcon from "@react-spectrum/s2/icons/ChevronDown";
import ChevronUpIcon from "@react-spectrum/s2/icons/ChevronUp";
import SearchIcon from "@react-spectrum/s2/icons/Search";
import { style } from "@react-spectrum/s2/style" with { type: "macro" };
import { $replaceMark, SearchInterviewExtension } from "./SearchInterviewExtension";
import { $isSearchResultNode } from "./SearchResultNode";

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
    const focusedResultValue = useExtensionSignalValue(SearchInterviewExtension, "focusedResult");
    const replacementValue = useExtensionSignalValue(SearchInterviewExtension, "replacement");
    const [editor] = useLexicalComposerContext();

    const spinner = (
        <ProgressCircle
            aria-label="Loading…"
            value={ 80 }
            isIndeterminate
            size="S"
            staticColor="white" />
    );

    // Both directions wrap. `%` after adding `resultCount` keeps the operand
    // non-negative --- JavaScript's `%` is a remainder, not a modulus, so a bare
    // `(current - 1) % n` yields -1 at the top of the list rather than n-1.
    //
    // The guard matters because `resultCount` now counts painted marks, which is
    // 0 whenever the query matches nothing; without it both handlers would
    // compute NaN and poison the signal. The buttons are disabled in that state,
    // but a keyboard shortcut bound to these later would not be.
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

    // Replace the focused match.
    //
    // Note what is NOT here: no tag. Every other `editor.update` in this
    // extension carries `history-merge` to tell PersistenceExtension "no text
    // changed, do not save" --- true of the highlight pass, which only wraps
    // runs in marks. Replace is the opposite and must stay untagged so that all
    // three downstream listeners fire: persistence saves the statement, history
    // records an undo step, and the re-search listener repaints the results.
    //
    // That last one is why nothing here re-runs the search by hand. Unwrapping
    // the mark dirties a leaf, the update listener sees an untagged commit with
    // dirty leaves, and the debounced search follows on its own.
    const replaceFocused = useCallback(() => {
        const index = focusedResult.peek();
        if (index === null) {
            return;
        }

        editor.update(() => {
            const key = resultKeys.peek()[index];
            if (key === undefined) {
                return;
            }

            const mark = $getNodeByKey(key) ?? undefined;
            if (!$isSearchResultNode(mark)) {
                return;
            }

            $replaceMark(mark, replacement.peek());
        });

        // Hold the index rather than advancing it. The replaced match leaves the
        // result set, so the NEXT match slides into this position --- keeping the
        // index puts the user on it, which is what repeated Replace clicks want.
        // Clamping against the new count happens in the highlight pass.
    }, [editor, focusedResult, resultKeys, replacement]);

    // Replace every match, in one update so it is one undo step and one save per
    // statement rather than one per occurrence.
    //
    // Iterating the key list is safe even though each `$replaceMark` unwraps a
    // mark: `resultKeys` is a plain array captured before the walk, and the keys
    // it names are independent nodes. Resolving each key inside the loop (rather
    // than resolving all the nodes up front) means a mark already removed as a
    // side effect of an earlier replacement simply misses.
    const replaceAll = useCallback(() => {
        const keys = resultKeys.peek();
        if (keys.length === 0) {
            return;
        }

        editor.update(() => {
            const value = replacement.peek();

            for (const key of keys) {
                const mark = $getNodeByKey(key) ?? undefined;
                if (!$isSearchResultNode(mark)) {
                    continue;
                }
                $replaceMark(mark, value);
            }
        });
    }, [editor, resultKeys, replacement]);
    return (
        <SearchContainer>
            <SearchFieldContainer>
                <TextField
                    aria-label="Search transcript"
                    type="search"
                    enterKeyHint="search"
                    inputMode="search"
                    prefix={ loading ? spinner : <SearchIcon /> }
                    size="M"
                    value={ queryValue ?? "" }
                    onChange={ value =>
                        // Writing the signal is the request; SearchDriver is what makes it a network call.
                        query.value = value.length > 0 ? value : null } />
            </SearchFieldContainer>
            <ButtonGroupContainer>
                { resultCount > 0 && (
                    <span style={{ padding: "0 1rem" }} className={ style({ color: "detail", fontSize: "detail" }) }>
                        { (focusedResultValue ?? 0) + 1 } of { resultCount }
                    </span>
                ) }
                <ActionButtonGroup isDisabled={ queryValue === null || loading || !resultCount }>
                    <ActionButton onPress={ focusPrevious }>
                        <ChevronUpIcon />
                        <Text>Previous</Text>
                    </ActionButton>
                    <ActionButton onPress={ focusNext }>
                        <ChevronDownIcon />
                        <Text>Next</Text>
                    </ActionButton>
                </ActionButtonGroup>
            </ButtonGroupContainer>
            <SearchFieldContainer>
                <TextField
                    aria-label="Replace with"
                    type="text"
                    inputMode="text"
                    size="M"
                    value={ replacementValue }
                    onChange={ value => replacement.value = value } />
            </SearchFieldContainer>
            <ButtonGroupContainer>
                { /* Deliberately not disabled on an empty replacement: clearing
                     a match is a legitimate edit, and "replace with nothing" is
                     how you delete a repeated filler word across a transcript. */ }
                <ActionButtonGroup isDisabled={ queryValue === null || loading || !resultCount }>
                    <ActionButton onPress={ replaceFocused }>
                        <Text>Replace</Text>
                    </ActionButton>
                    <ActionButton onPress={ replaceAll }>
                        <Text>Replace All</Text>
                    </ActionButton>
                </ActionButtonGroup>
            </ButtonGroupContainer>
        </SearchContainer>
    );
}
