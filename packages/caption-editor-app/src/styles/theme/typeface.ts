import { css } from 'styled-components'

import {
  accent as plumberAccent,
  mono as plumberMono,
  primary as plumberPrimary,
  PlumberProps,
} from './plumber'

export const accentFamily = css`
  font-family:
    'Charlie',
    'Georgia',
    'Garamond',
    'Times New Roman',
    'Times',
    serif !important;
`

export const accent = (plumberOpts: Partial<PlumberProps> = {}) => css`
  ${ accentFamily }
  ${ plumberAccent(plumberOpts) }
`

export const primaryFamily = css`
  font-family:
    'Maison Neue',
    -apple-system,
    BlinkMacSystemFont,
    'Helvetica Neue',
    'Helvetica',
    sans-serif !important;
`

export const primary = (plumberOpts: Partial<PlumberProps> = {}) => css`
  ${ primaryFamily }
  ${ plumberPrimary(plumberOpts) }
`

export const monoFamily = css`
  font-family:
    'PragmataPro',
    'Courier New',
    Courier,
    monospace !important;
`

export const mono = css`
  ${ monoFamily }
  ${ plumberMono() }
`
