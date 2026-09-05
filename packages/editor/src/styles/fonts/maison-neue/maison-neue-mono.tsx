import { createGlobalStyle, css } from "styled-components";

const fontFaces = css`
  @font-face {
    font-weight: 400;
    font-family: 'Maison Neue Mono';
    font-style: normal;
    src: url('/assets/fonts/maison-neue-mono/MaisonNeueMono-Regular.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 400;
    font-family: 'Maison Neue Mono';
    font-style: italic;
    src: url('/assets/fonts/maison-neue-mono/MaisonNeueMono-Italic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'Maison Neue Mono';
    font-style: normal;
    src: url('/assets/fonts/maison-neue-mono/MaisonNeueMono-Bold.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'Maison Neue Mono';
    font-style: italic;
    src: url('/assets/fonts/maison-neue-mono/MaisonNeueMono-BoldItalic.woff2') format('woff2');

    font-display: swap;
  }
`;

export const MaisonNeueMono = createGlobalStyle`
  ${ fontFaces }
`;
