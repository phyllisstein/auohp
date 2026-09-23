import { createGlobalStyle } from "styled-components";


// Styles for the statement wrapper, its non-editable chrome column, and the
// editable content element (see StatementNode.createDOM / getDOMSlot). Global
// because the class names are stamped by Lexical's own DOM building, not by a
// component that could own a scoped style.
export const StatementStyles = createGlobalStyle`
    .auohp-statement {
        display: flex;
        gap: 0.75rem;
        align-items: flex-start;
        padding: 0.25rem 0;
    }

    .auohp-statement__chrome {
        /* Seeking is a click on this column specifically (see
           StatementSeekExtension), so the chrome has to advertise itself as the
           target --- otherwise the only way to discover the gesture is to
           perform it by accident. */
        cursor: pointer;
        user-select: none;

        display: flex;
        flex-direction: column;
        flex-shrink: 0;

        min-width: 6rem;

        font-family: monospace;
        font-size: 0.75rem;
        color: #888;

        transition: color 0.12s ease;

        &:hover {
            color: #333;
        }
    }

    .auohp-statement__content {
        flex: 1;
    }
`;
