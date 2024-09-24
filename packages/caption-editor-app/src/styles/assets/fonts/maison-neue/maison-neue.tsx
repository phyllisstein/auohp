import {createGlobalStyle, css} from 'styled-components'

const fontFaces = css`
  @font-face {
    font-weight: 100;
    font-family: 'Maison Neue';
    font-style: normal;
    src: url('/assets/fonts/maison-neue/MaisonNeue-ExtraThin.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 100;
    font-family: 'Maison Neue';
    font-style: italic;
    src: url('/assets/fonts/maison-neue/MaisonNeue-ExtraThinItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 200;
    font-family: 'Maison Neue';
    font-style: normal;
    src: url('/assets/fonts/maison-neue/MaisonNeue-Thin.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 200;
    font-family: 'Maison Neue';
    font-style: italic;
    src: url('/assets/fonts/maison-neue/MaisonNeue-ThinItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 300;
    font-family: 'Maison Neue';
    font-style: normal;
    src: url('/assets/fonts/maison-neue/MaisonNeue-Light.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 300;
    font-family: 'Maison Neue';
    font-style: italic;
    src: url('/assets/fonts/maison-neue/MaisonNeue-LightItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 400;
    font-family: 'Maison Neue';
    font-style: normal;
    src: url('/assets/fonts/maison-neue/MaisonNeue-Book.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 400;
    font-family: 'Maison Neue';
    font-style: italic;
    src: url('/assets/fonts/maison-neue/MaisonNeue-BookItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 500;
    font-family: 'Maison Neue';
    font-style: normal;
    src: url('/assets/fonts/maison-neue/MaisonNeue-Medium.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 500;
    font-family: 'Maison Neue';
    font-style: italic;
    src: url('/assets/fonts/maison-neue/MaisonNeue-MediumItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 600;
    font-family: 'Maison Neue';
    font-style: normal;
    src: url('/assets/fonts/maison-neue/MaisonNeue-Demi.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 600;
    font-family: 'Maison Neue';
    font-style: italic;
    src: url('/assets/fonts/maison-neue/MaisonNeue-DemiItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'Maison Neue';
    font-style: normal;
    src: url('/assets/fonts/maison-neue/MaisonNeue-Bold.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'Maison Neue';
    font-style: italic;
    src: url('/assets/fonts/maison-neue/MaisonNeue-BoldItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 800;
    font-family: 'Maison Neue';
    font-style: normal;
    src: url('/assets/fonts/maison-neue/MaisonNeue-ExtraBold.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 800;
    font-family: 'Maison Neue';
    font-style: italic;
    src: url('/assets/fonts/maison-neue/MaisonNeue-ExtraBoldItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 900;
    font-family: 'Maison Neue';
    font-style: normal;
    src: url('/assets/fonts/maison-neue/MaisonNeue-Black.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 900;
    font-family: 'Maison Neue';
    font-style: italic;
    src: url('/assets/fonts/maison-neue/MaisonNeue-BlackItalic.woff2') format('woff2');

    font-display: swap;
  }
`

export const MaisonNeue = createGlobalStyle`
  ${fontFaces}
`
