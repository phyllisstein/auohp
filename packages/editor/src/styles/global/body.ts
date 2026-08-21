import { createGlobalStyle } from "styled-components";

export const Body = createGlobalStyle`
    html {
        box-sizing: border-box;
        margin: 0;
        padding: 0;

        font-family:
            "Adobe Clean",
            -apple-system,
            BlinkMacSystemFont,
            "Segoe UI",
            Roboto,
            "Helvetica Neue",
            Arial,
            "Noto Sans",
            sans-serif,
            "Apple Color Emoji",
            "Segoe UI Emoji",
            "Segoe UI Symbol",
            "Noto Color Emoji" !important;
        font-size: 112.5%;
        font-variant-ligatures: common-ligatures;
        font-variant-numeric: lining-nums proportional-nums;
        font-kerning: normal;
        text-rendering: geometricprecision;
    }
`;
