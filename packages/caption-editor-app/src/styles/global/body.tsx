import { createGlobalStyle } from 'styled-components'

export const Body = createGlobalStyle`
  *,
  *::before,
  *::after {
    box-sizing: inherit;

    -webkit-tap-highlight-color: rgba(255, 255, 255, 0);
    text-rendering: geometricPrecision;
  }

  html {
    box-sizing: border-box;
    margin: 0;
    padding: 0;

    font-size: 16px;
    font-family: 'Adobe Clean', -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', 'Helvetica Neue', 'Arial', 'Noto Sans', sans-serif, 'Apple Color Emoji', 'Segoe UI Emoji', 'Segoe UI Symbol', 'Noto Color Emoji';

    font-kerning: normal;
    font-variant-ligatures: common-ligatures;
    font-variant-numeric: oldstyle-nums proportional-nums;
  }
`
