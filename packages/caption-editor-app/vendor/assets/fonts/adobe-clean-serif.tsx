'use client'

import { createGlobalStyle, css } from 'styled-components'

const fontFaces = css`
    @font-face {
        font-weight: 400;
        font-family: 'Adobe Clean Serif';
        font-style: normal;
        src: url('/assets/fonts/adobe-clean-serif/AdobeCleanSerif-Regular.woff2')
            format('woff2');

        font-display: swap;

    }

    @font-face {
        font-weight: 400;
        font-family: 'Adobe Clean Serif';
        font-style: italic;
        src: url('/assets/fonts/adobe-clean-serif/AdobeCleanSerif-It.woff2')
            format('woff2');

        font-display: swap;
    }

    @font-face {
        font-weight: 500;
        font-family: 'Adobe Clean Serif';
        font-style: normal;
        src: url('/assets/fonts/adobe-clean-serif/AdobeCleanSerif-Medium.woff2')
            format('woff2');

        font-display: swap;
    }

    @font-face {
        font-weight: 500;
        font-family: 'Adobe Clean Serif';
        font-style: italic;
        src: url('/assets/fonts/adobe-clean-serif/AdobeCleanSerif-MediumIt.woff2')
            format('woff2');

        font-display: swap;
    }

    @font-face {
        font-weight: 700;
        font-family: 'Adobe Clean Serif';
        font-style: normal;
        src: url('/assets/fonts/adobe-clean-serif/AdobeCleanSerif-Bold.woff2')
            format('woff2');

        font-display: swap;
    }

    @font-face {
        font-weight: 700;
        font-family: 'Adobe Clean Serif';
        font-style: italic;
        src: url('/assets/fonts/adobe-clean-serif/AdobeCleanSerif-BoldIt.woff2')
            format('woff2');

        font-display: swap;
    }

    @font-face {
        font-weight: 900;
        font-family: 'Adobe Clean Serif';
        font-style: normal;
        src: url('/assets/fonts/adobe-clean-serif/AdobeCleanSerif-Black.woff2')
            format('woff2');

        font-display: swap;
    }

    @font-face {
        font-weight: 900;
        font-family: 'Adobe Clean Serif';
        font-style: italic;
        src: url('/assets/fonts/adobe-clean-serif/AdobeCleanSerif-BlackIt.woff2')
            format('woff2');

        font-display: swap;
    }
`

export const AdobeCleanSerif = createGlobalStyle`
    ${ fontFaces }
`
