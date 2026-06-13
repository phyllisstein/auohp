import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { createEditor } from "slate";
import { Slate, Editable, withReact } from "slate-react";
import { Button } from "@react-spectrum/s2/Button";
import { createLink } from "@tanstack/react-router";

const ButtonLink = createLink(Button);


export const Route = createFileRoute("/")({
    component: Page,
});


const initialValue = [
    {
        type: "paragraph",
        children: [{ text: "A line of text in a paragraph." }],
    },
    {
        type: "statement",
        children: [
            { type: "word", children: [{ text: "ACT" }], startTime: 0, endTime: 1000 },
            { type: "word", children: [{ text: "UP" }], startTime: 1000, endTime: 2000 },
        ],
    },
];


function Page() {
    const [editor] = useState(() => withReact(createEditor()));

    return (
        <>
            <Slate editor={ editor } initialValue={ initialValue }>
                <Editable />
            </Slate>
            <ButtonLink to="/oops" variant="accent">Save</ButtonLink>
        </>
    );
}
