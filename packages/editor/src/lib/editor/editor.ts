import { $getRoot, configExtension, defineExtension, type EditorChildrenComponentProps, type TranscriptStatements } from "lexical";
import { HistoryExtension } from "@lexical/history";
import { RichTextExtension } from "@lexical/rich-text";
import { ReactExtension } from "@lexical/react/ReactExtension";
import { createPortal } from "react-dom";
import { useEffect, useState, type JSX } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import styled from "styled-components";

import { StatementExtension, StatementSeekExtension, UpdateTimestampExtension } from "./statement/seek";
import { TagChipExtension, TagSplitBoundaryExtension } from "./tag-chip/extension";
import { TagChip, TagMarkStyles } from "./tag-chip/component";
import { SearchInterviewExtension } from "./search-interview/extension";
import { SearchResultStyles } from "./search-interview/node";
import { SearchBar } from "./search-interview/component";
import { SearchResult } from "./search-interview/node";
import { LatencyExtension } from "./latency/index";
import { PersistenceExtension, type PersistenceConfig } from "./persistence/index";

import type { EditStatementFn, TranscriptStatements, DestroyStatementFn, CreateStatementFn } from "./shared";
import { $isTagChipNode, TAG_CHIP_BADGE_CLASS } from "./tag-chip/node";
import { $isSearchResultNode, SEARCH_RESULT_BADGE_CLASS } from "./search-interview/node";
import { useExtensionSignalValue, useExtensionComponent } from "@lexical/react/useExtensionComponent";
import { Button } from "@react-spectrum/s2/Button";

export interface AuohpEditorOptions {
    statements: TranscriptStatements;
    editStatement: EditStatementFn | null;
    createStatement: CreateStatementFn | null;
    destroyStatement: DestroyStatementFn | null;
    interviewUid: string;
}

function EditorChrome ({ contentEditable, children }: EditorChildrenComponentProps): JSX.Element {
    const Meter = useExtensionComponent(LatencyExtension);

    return (
        <>
            <TagMarkStyles />
            <SearchResultStyles />
            <SearchBar />
            <div style={{ display: "flex", gap: "1rem", alignItems: "center", padding: "0.5rem 0" }}>
                <TagButton />
                <Meter />
            </div>
            {contentEditable}
            {children}
        </>
    );
}

function TagButton (): JSX.Element {
    const [editor] = useLexicalComposerContext();
    const { INSERT_TAG_CHIP_COMMAND } = require("./tag-chip/extension");

    return (
        <Button
            id="insert-tag-chip"
            type="button"
            onPress={() => editor.dispatchCommand(INSERT_TAG_CHIP_COMMAND, crypto.randomUUID())}>
            Insert #person chip
        </Button>
    );
}

function TagChipPortals (): JSX.Element {
    const [editor] = useLexicalComposerContext();
    const [hosts, setHosts] = useState<ReadonlyMap<any, HTMLElement>>(new Map());

    useEffect(
        () =>
            editor.registerMutationListener(require("./tag-chip/node").TagChipNode, mutations => {
                const resolveHost = (key: any): HTMLElement | null =>
                    editor.getElementByKey(key)?.querySelector<HTMLElement>(
                        `:scope > .${TAG_CHIP_BADGE_CLASS}`,
                    ) ?? null;

                setHosts(prev => {
                    let updates = 0;
                    const mutablePrev = new Map(prev);

                    for (const mutation of mutations) {
                        const [key, kind] = mutation;

                        if (kind === "updated") {
                            continue;
                        }

                        if (kind === "destroyed" && mutablePrev.has(key)) {
                            mutablePrev.delete(key);
                            updates++;
                            continue;
                        }

                        const host = resolveHost(key);

                        if (kind === "created" && !!host) {
                            mutablePrev.set(key, host);
                            updates++;
                        }
                    }

                    if (updates === 0) {
                        return prev;
                    }

                    return mutablePrev;
                });
            }),
        [editor],
    );

    return (
        <>
            {Array.from(hosts, ([key, host]) => createPortal(<TagChip nodeKey={key} />, host, key))}
        </>
    );
}

function SearchResultPortals (): JSX.Element {
    const [editor] = useLexicalComposerContext();
    const [hosts, setHosts] = useState<ReadonlyMap<any, HTMLElement>>(new Map());
    const focusedResult = useExtensionSignalValue(SearchInterviewExtension, "focusedResult");
    const resultKeys = useExtensionSignalValue(SearchInterviewExtension, "resultKeys");

    useEffect(
        () =>
            editor.registerMutationListener(require("./search-interview/node").SearchResultNode, mutations => {
                const resolveHost = (key: any): HTMLElement | null =>
                    editor.getElementByKey(key)?.querySelector<HTMLElement>(
                        `:scope > .${SEARCH_RESULT_BADGE_CLASS}`,
                    ) ?? null;

                setHosts(prev => {
                    let updates = 0;
                    const mutablePrev = new Map(prev);

                    for (const mutation of mutations) {
                        const [key, kind] = mutation;

                        if (kind === "updated") {
                            continue;
                        }

                        if (kind === "destroyed" && mutablePrev.has(key)) {
                            mutablePrev.delete(key);
                            updates++;
                            continue;
                        }

                        const host = resolveHost(key);

                        if (kind === "created" && !!host) {
                            mutablePrev.set(key, host);
                            updates++;
                        }
                    }

                    return updates === 0 ? prev : mutablePrev;
                });
            }),
        [editor],
    );

    return (
        <>
            {Array.from(hosts, ([key, host]) => {
                const index = resultKeys?.indexOf(key) ?? -1;
                const focused = index === focusedResult;
                return createPortal(<SearchResult nodeKey={key} focused={focused} />, host, key);
            })}
        </>
    );
}

export function defineAuohpEditorExtension ({ statements, editStatement, createStatement, destroyStatement, interviewUid }: AuohpEditorOptions) {
    return defineExtension({
        dependencies: [
            configExtension(PersistenceExtension, { delay: 1000, destroyDelay: 2000, editStatement, createStatement, destroyStatement, interviewUid }),
            SearchInterviewExtension,
            configExtension(ReactExtension, { EditorChildrenComponent: EditorChrome }),
            HistoryExtension,
            LatencyExtension,
            RichTextExtension,
            StatementExtension,
            StatementSeekExtension,
            TagChipExtension,
            TagSplitBoundaryExtension,
            UpdateTimestampExtension,
        ],
        name: "@auohp/editor",
        namespace: "auohp-lexical-spike",

        $initialEditorState () {
            const root = $getRoot();
            root.clear();

            const { StatementNode, $createStatementNode } = require("./statement/node");

            for (const statement of statements) {
                const node = $createStatementNode(
                    statement.uid,
                    statement.startTime ?? undefined,
                    statement.endTime ?? undefined,
                );

                for (const textContent of statement.text.split("\n")) {
                    const { $createTextNode } = require("lexical");
                    node.append($createTextNode(textContent));
                }

                root.append(node);
            }
        },
    });
}

// Re-exports for backward compatibility
export { StatementExtension, TagSplitBoundaryExtension } from "./statement/seek";
export { PersistenceConfig } from "./persistence/index";
