import { createGlobalStyle, css } from 'styled-components'

const fontFaces = css`
  @font-face {
    font-weight: 100;
    font-family: 'Charlie';
    font-style: normal;
    src: url('/assets/fonts/charlie/CharliePro-Hairline.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 100;
    font-family: 'Charlie';
    font-style: italic;
    src: url('/assets/fonts/charlie/CharliePro-HairlineItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 200;
    font-family: 'Charlie';
    font-style: normal;
    src: url('/assets/fonts/charlie/CharliePro-Thin.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 200;
    font-family: 'Charlie';
    font-style: italic;
    src: url('/assets/fonts/charlie/CharliePro-ThinItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 300;
    font-family: 'Charlie';
    font-style: normal;
    src: url('/assets/fonts/charlie/CharliePro-Light.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 300;
    font-family: 'Charlie';
    font-style: italic;
    src: url('/assets/fonts/charlie/CharliePro-LightItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 400;
    font-family: 'Charlie';
    font-style: normal;
    src: url('/assets/fonts/charlie/CharliePro-Regular.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 400;
    font-family: 'Charlie';
    font-style: italic;
    src: url('/assets/fonts/charlie/CharliePro-RegularItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 500;
    font-family: 'Charlie';
    font-style: normal;
    src: url('/assets/fonts/charlie/CharliePro-Medium.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 500;
    font-family: 'Charlie';
    font-style: italic;
    src: url('/assets/fonts/charlie/CharliePro-MediumItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 600;
    font-family: 'Charlie';
    font-style: normal;
    src: url('/assets/fonts/charlie/CharliePro-Semibold.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 600;
    font-family: 'Charlie';
    font-style: italic;
    src: url('/assets/fonts/charlie/CharliePro-SemiboldItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'Charlie';
    font-style: normal;
    src: url('/assets/fonts/charlie/CharliePro-Bold.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'Charlie';
    font-style: italic;
    src: url('/assets/fonts/charlie/CharliePro-BoldItalic.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 900;
    font-family: 'Charlie';
    font-style: normal;
    src: url('/assets/fonts/charlie/CharliePro-Black.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 900;
    font-family: 'Charlie';
    font-style: italic;
    src: url('/assets/fonts/charlie/CharliePro-BlackItalic.woff2') format('woff2');

    font-display: swap;
  }
`

export const Charlie = createGlobalStyle`
  ${ fontFaces }
`
