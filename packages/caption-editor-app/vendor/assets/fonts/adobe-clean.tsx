import { createGlobalStyle, css } from 'styled-components'

const fontFaces = css`
  @font-face {
    font-weight: 200;
    font-family: 'Adobe Clean';
    font-style: normal;
    font-stretch: normal;
    src: url('/assets/fonts/adobe-clean/AdobeClean-Light.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 200;
    font-family: 'Adobe Clean';
    font-style: italic;
    font-stretch: normal;
    src: url('/assets/fonts/adobe-clean/AdobeClean-LightIt.woff2')
      format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 300;
    font-family: 'Adobe Clean';
    font-style: normal;
    font-stretch: normal;
    src: url('/assets/fonts/adobe-clean/AdobeClean-SemiLight.woff2')
      format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 300;
    font-family: 'Adobe Clean';
    font-style: italic;
    font-stretch: normal;
    src: url('/assets/fonts/adobe-clean/AdobeClean-SemiLightIt.woff2')
      format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 400;
    font-family: 'Adobe Clean';
    font-style: normal;
    font-stretch: normal;
    src: url('/assets/fonts/adobe-clean/AdobeClean-Regular.woff2')
      format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 400;
    font-family: 'Adobe Clean';
    font-style: italic;
    font-stretch: normal;
    src: url('/assets/fonts/adobe-clean/AdobeClean-It.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'Adobe Clean';
    font-style: normal;
    font-stretch: normal;
    src: url('/assets/fonts/adobe-clean/AdobeClean-Bold.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'Adobe Clean';
    font-style: italic;
    font-stretch: normal;
    src: url('/assets/fonts/adobe-clean/AdobeClean-BoldIt.woff2')
      format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 800;
    font-family: 'Adobe Clean';
    font-style: normal;
    font-stretch: normal;
    src: url('/assets/fonts/adobe-clean/AdobeClean-ExtraBold.woff2')
      format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 800;
    font-family: 'Adobe Clean';
    font-style: italic;
    font-stretch: normal;
    src: url('/assets/fonts/adobe-clean/AdobeClean-ExtraBoldIt.woff2')
      format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 900;
    font-family: 'Adobe Clean';
    font-style: normal;
    font-stretch: normal;
    src: url('/assets/fonts/adobe-clean/AdobeClean-Black.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 900;
    font-family: 'Adobe Clean';
    font-style: italic;
    font-stretch: normal;
    src: url('/assets/fonts/adobe-clean/AdobeClean-BlackIt.woff2')
      format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 400;
    font-family: 'Adobe Clean';
    font-style: normal;
    font-stretch: condensed;
    src: url('/assets/fonts/adobe-clean/AdobeClean-Cond.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 400;
    font-family: 'Adobe Clean';
    font-style: italic;
    font-stretch: condensed;
    src: url('/assets/fonts/adobe-clean/AdobeClean-CondIt.woff2')
      format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'Adobe Clean';
    font-style: normal;
    font-stretch: condensed;
    src: url('/assets/fonts/adobe-clean/AdobeClean-BoldCond.woff2')
      format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'Adobe Clean';
    font-style: italic;
    font-stretch: condensed;
    src: url('/assets/fonts/adobe-clean/AdobeClean-BoldCondIt.woff2')
      format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 400;
    font-family: 'Adobe Clean';
    font-style: normal;
    font-stretch: semi-condensed;
    src: url('/assets/fonts/adobe-clean/AdobeClean-SemiCn.woff2')
      format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 400;
    font-family: 'Adobe Clean';
    font-style: italic;
    font-stretch: semi-condensed;
    src: url('/assets/fonts/adobe-clean/AdobeClean-SemiCnIt.woff2')
      format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'Adobe Clean';
    font-style: normal;
    font-stretch: semi-condensed;
    src: url('/assets/fonts/adobe-clean/AdobeClean-BoldSemiCn.woff2')
      format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'Adobe Clean';
    font-style: italic;
    font-stretch: semi-condensed;
    src: url('/assets/fonts/adobe-clean/AdobeClean-BoldSemiCnIt.woff2')
      format('woff2');

    font-display: swap;
  }
`

export const AdobeClean = createGlobalStyle`
  ${fontFaces}
`
