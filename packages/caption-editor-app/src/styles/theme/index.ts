import * as animation from './animation'
import * as ease from './ease'
import * as elevation from './elevation'
import * as paletteDark from './palette-spectrum-dark'
import * as paletteLight from './palette-spectrum-light'
import * as responsive from './responsive'

export const theme = {
    animation,
    ease,
    elevation,
    palette: paletteDark,
    paletteDark,
    paletteLight,
    responsive,
}

type CustomTheme = typeof theme

declare module 'styled-components' {
    // eslint-disable-next-line @typescript-eslint/no-empty-interface
    export interface DefaultTheme extends CustomTheme {}
}
