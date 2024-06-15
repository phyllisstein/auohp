'use client'

import { createGlobalStyle } from 'styled-components'

export const Preflight = createGlobalStyle`
  *,
  *::before,
  *::after {
    box-sizing: border-box;
  }

  :root {
    tab-size: 4;
  }

  html {
    font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', 'Helvetica Neue', 'Arial', 'Noto Sans', sans-serif, 'Apple Color Emoji', 'Segoe UI Emoji', 'Segoe UI Symbol', 'Noto Color Emoji';
    line-height: 1.5;

    text-size-adjust: 100%;
  }

  hr {
    height: 0;

    color: inherit;

    border-top-width: 1px;
  }

  abbr[title] {
    text-decoration: underline dotted;
  }

  b,
  strong {
    font-weight: bolder;
  }

  code,
  kbd,
  samp,
  pre {
    font-size: 1em;
    font-family: ui-monospace, 'SFMono-Regular', 'Consolas', 'Liberation Mono', 'Menlo', monospace;
  }

  small {
    font-size: 80%;
  }

  sub,
  sup {
    position: relative;

    font-size: 75%;
    line-height: 0;
    vertical-align: baseline;
  }

  sub {
    bottom: -0.25em;
  }

  sup {
    top: -0.5em;
  }

  table {
    text-indent: 0;

    border-color: inherit;
    border-collapse: collapse;
  }

  button,
  input,
  optgroup,
  select,
  textarea {
    margin: 0;

    font-size: 100%;
    font-family: inherit;
    line-height: 1.15;
    text-transform: none;
  }

  button,
  [type='button'],
  [type='reset'],
  [type='submit'] {
    appearance: button;
  }

  ::-moz-focus-inner {
    padding: 0;
    border-style: none;
  }

  :-moz-focusring {
    outline: 1px dotted ButtonText;
  }

  :-moz-ui-invalid {
    box-shadow: none;
  }

  legend {
    padding: 0;
  }

  progress {
    vertical-align: baseline;
  }

  ::-webkit-inner-spin-button,
  ::-webkit-outer-spin-button {
    height: auto;
  }

  [type='search'] {
    outline-offset: -2px;
    appearance: textfield;
  }

  ::-webkit-search-decoration {
    appearance: none;
  }

  ::-webkit-file-upload-button {
    font: inherit;
    appearance: button;
  }

  summary {
    display: list-item;
  }

  blockquote,
  dl,
  dd,
  h1,
  h2,
  h3,
  h4,
  h5,
  h6,
  hr,
  figure,
  p,
  pre {
    margin: 0;
  }

  button {
    background-color: transparent;
    background-image: none;
  }

  button:focus {
    outline: 1px dotted;
    outline: 5px auto -webkit-focus-ring-color;
  }

  button,
  [role='button'] {
    padding: 0;

    color: inherit;
    line-height: inherit;

    cursor: pointer;
  }

  input,
  optgroup,
  select,
  textarea {
    padding: 0;

    color: inherit;
    line-height: inherit;
  }

  fieldset {
    margin: 0;
    padding: 0;
  }

  ol,
  ul {
    margin: 0;
    padding: 0;

    list-style: none;
  }

  body {
    margin: 0;

    font-family: system-ui, -apple-system, 'Segoe UI', 'Roboto', 'Helvetica', 'Arial', sans-serif, 'Apple Color Emoji', 'Segoe UI Emoji';
    line-height: inherit;
  }

  *,
  ::before,
  ::after {
    box-sizing: border-box;

    border-color: #E5E7EB;
    border-style: solid;
    border-width: 0;
  }

  img {
    border-style: solid;
  }

  textarea {
    resize: vertical;
  }

  input::placeholder,
  textarea::placeholder {
    color: #9CA3AF;
    opacity: 1;
  }

  h1,
  h2,
  h3,
  h4,
  h5,
  h6 {
    font-weight: inherit;
    font-size: inherit;
  }

  a {
    color: inherit;
    text-decoration: inherit;
  }

  code,
  kbd,
  samp {
    font-family: ui-monospace, 'SFMono-Regular', 'Menlo', 'Monaco', 'Consolas', 'Liberation Mono', 'Courier New', monospace;
  }

  img,
  svg,
  video,
  canvas,
  audio,
  iframe,
  embed,
  object {
    display: block;
    vertical-align: middle;
  }

  img,
  video {
    max-width: 100%;
    height: auto;
  }

`
