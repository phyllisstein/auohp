import {createGlobalStyle, css} from 'styled-components'

const fontFaces = css`
  @font-face {
    font-weight: 400;
    font-family: 'PragmataPro';
    font-style: normal;
    src: url('/assets/assets/fonts/pragmatapro/PragmataProR_liga_0830_Script.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 400;
    font-family: 'PragmataPro';
    font-style: italic;
    src: url('/assets/assets/fonts/pragmatapro/PragmataProI_0830_Script.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'PragmataPro';
    font-style: normal;
    src: url('/assets/assets/fonts/pragmatapro/PragmataProB_0830_Script.woff2') format('woff2');

    font-display: swap;
  }

  @font-face {
    font-weight: 700;
    font-family: 'PragmataPro';
    font-style: italic;
    src: url('/assets/assets/fonts/pragmatapro/PragmataProZ_0830_Script.woff2') format('woff2');

    font-display: swap;
  }
`

export const PragmataPro = createGlobalStyle`
  ${fontFaces}
`
