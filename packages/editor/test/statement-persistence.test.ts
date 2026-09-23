import { afterEach, describe, expect, it, vi } from "vitest";
import {
    $createTextNode,
    $getRoot,
    $getSelection,
    $isRangeSelection,
    $isTextNode,
    configExtension,
    defineExtension,
    type LexicalEditor,
} from "lexical";
import { buildEditorFromExtensions } from "@lexical/extension";
import { Playhead, type PlayheadModel } from "~/playhead";
import { PersistenceExtension, type EditStatementFn } from "~/lexical/persistence/PersistenceExtension";
import { SYNTHETIC_UID_MARKER } from "~/lexical/persistence/synthetic-uid";
import { StatementExtension } from "~/lexical/statement/StatementExtension";
import { $createStatementNode, $isStatementNode } from "~/lexical/statement/StatementNode";

// Editor-level checks for two fixes backported from the Svelte port, each built
// from the smallest extension graph that exhibits it --- deliberately without
// TagChipExtension or SearchInterviewExtension, whose mutation listeners used to
// mask the persistence bug.

const settle = (ms = 0) => new Promise(resolve => setTimeout(resolve, ms));

describe("statement + persistence", () => {
    let editor: LexicalEditor | undefined;
    let container: HTMLElement | undefined;

    afterEach(() => {
        editor?.dispose();
        container?.remove();
    });

    const build = async (playhead: PlayheadModel, editStatement: EditStatementFn | null = null) => {
        const extension = defineExtension({
            name: "@auohp/test",
            dependencies: [
                configExtension(StatementExtension, { playhead }),
                configExtension(PersistenceExtension, { delay: 5, destroyDelay: 5, editStatement, interviewUid: "interview" }),
            ],
            $initialEditorState () {
                const statement = $createStatementNode("s1", 10, 20);
                statement.append($createTextNode("We shut it down"));
                $getRoot().append(statement);
            },
        });

        editor = buildEditorFromExtensions(extension);
        container = document.createElement("div");
        container.contentEditable = "true";
        document.body.append(container);
        editor.setRootElement(container);
        await settle();
        return editor;
    };

    it("refuses to build without a playhead", () => {
        const extension = defineExtension({ name: "@auohp/test-no-playhead", dependencies: [StatementExtension] });
        expect(() => buildEditorFromExtensions(extension)).toThrow(/no playhead/);
    });

    it("splits at the editor's own playhead position", async () => {
        const playhead = new Playhead();
        playhead.timestamp.value = 14;
        const ed = await build(playhead);

        ed.update(() => {
            const text = $getRoot().getFirstDescendant();
            if ($isTextNode(text)) {
                text.select(7, 7);
            }
            const selection = $getSelection();
            if ($isRangeSelection(selection)) {
                selection.insertParagraph();
            }
        }, { discrete: true });

        const [first, second] = ed.read(() => $getRoot().getChildren());
        ed.read(() => {
            expect($isStatementNode(first) && first.getEndTime()).toBe(14);
            expect($isStatementNode(second) && second.getStartTime()).toBe(14);
            expect($isStatementNode(second) && second.getEndTime()).toBe(20);
            expect($isStatementNode(second) && second.getUid()).toContain(SYNTHETIC_UID_MARKER);
        });
    });

    it("persists edits with no other mutation listener in the graph", async () => {
        const editStatement = vi.fn();
        const ed = await build(new Playhead(), editStatement as unknown as EditStatementFn);

        // The seed is tagged history-merge and must not be saved.
        expect(editStatement).not.toHaveBeenCalled();

        ed.update(() => {
            const text = $getRoot().getFirstDescendant();
            if ($isTextNode(text)) {
                text.setTextContent("We shut it all down");
            }
        }, { discrete: true });
        await settle(50);

        expect(editStatement).toHaveBeenCalledTimes(1);
        expect(editStatement.mock.calls[0][0].variables).toEqual({
            uid: "s1",
            text: "We shut it all down",
            startTime: 10,
            endTime: 20,
        });
    });
});
