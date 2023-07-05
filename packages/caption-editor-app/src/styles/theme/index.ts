import * as paletteDark from './palette-spectrum-dark'
import * as paletteLight from './palette-spectrum-light'

export const theme = {
  palette: paletteDark,
  paletteDark,
  paletteLight,
}

type CustomTheme = typeof theme

declare module 'styled-components' {
  // eslint-disable-next-line @typescript-eslint/no-empty-interface
  export interface DefaultTheme extends CustomTheme {}
}
