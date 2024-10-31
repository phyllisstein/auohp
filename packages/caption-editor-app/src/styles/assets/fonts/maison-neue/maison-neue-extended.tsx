import { createGlobalStyle, css } from 'styled-components'

const fontFaces = css`
  @font-face {
    font-weight: 100;
    font-family: 'Maison Neue';
    font-style: normal;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-ExtraThin.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 100;
    font-family: 'Maison Neue';
    font-style: italic;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-ExtraThinItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 200;
    font-family: 'Maison Neue';
    font-style: normal;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-Thin.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 200;
    font-family: 'Maison Neue';
    font-style: italic;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-ThinItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 300;
    font-family: 'Maison Neue';
    font-style: normal;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-Light.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 300;
    font-family: 'Maison Neue';
    font-style: italic;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-LightItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 400;
    font-family: 'Maison Neue';
    font-style: normal;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-Book.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 400;
    font-family: 'Maison Neue';
    font-style: italic;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-BookItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 500;
    font-family: 'Maison Neue';
    font-style: normal;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-Medium.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 500;
    font-family: 'Maison Neue';
    font-style: italic;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-MediumItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 600;
    font-family: 'Maison Neue';
    font-style: normal;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-Demi.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 600;
    font-family: 'Maison Neue';
    font-style: italic;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-DemiItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'Maison Neue';
    font-style: normal;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-Bold.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'Maison Neue';
    font-style: italic;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-BoldItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 800;
    font-family: 'Maison Neue';
    font-style: normal;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-ExtraBold.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 800;
    font-family: 'Maison Neue';
    font-style: italic;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-ExtraBoldItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 900;
    font-family: 'Maison Neue';
    font-style: normal;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-Black.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 900;
    font-family: 'Maison Neue';
    font-style: italic;
    font-stretch: expanded;
    src: url('/assets/fonts/maison-neue-extended/MaisonNeueExtended-BlackItalic.woff2') format('woff2');

    font-display: swap;
  }
`

export const MaisonNeueExtended = createGlobalStyle`
  ${ fontFaces }
`
