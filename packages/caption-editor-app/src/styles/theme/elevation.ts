import * as R from 'ramda'
import { css } from 'styled-components'

import { css as animationCSS, AnimationDuration } from './animation'

const SHADOWS = {
  AMBIENT: [
    '0px 0px 0px 0px',
    '0px 1px 3px 0px',
    '0px 1px 5px 0px',
    '0px 1px 8px 0px',
    '0px 1px 10px 0px',
    '0px 1px 14px 0px',
    '0px 1px 18px 0px',
    '0px 2px 16px 1px',
    '0px 3px 14px 2px',
    '0px 3px 16px 2px',
    '0px 4px 18px 3px',
    '0px 4px 20px 3px',
    '0px 5px 22px 4px',
    '0px 5px 24px 4px',
    '0px 5px 26px 4px',
    '0px 6px 28px 5px',
    '0px 6px 30px 5px',
    '0px 6px 32px 5px',
    '0px 7px 34px 6px',
    '0px 7px 36px 6px',
    '0px 8px 38px 7px',
    '0px 8px 40px 7px',
    '0px 8px 42px 7px',
    '0px 9px 44px 8px',
    '0px 9px 46px 8px',
  ],
  PENUMBRA: [
    '0px 0px 0px 0px',
    '0px 1px 1px 0px',
    '0px 2px 2px 0px',
    '0px 3px 4px 0px',
    '0px 4px 5px 0px',
    '0px 5px 8px 0px',
    '0px 6px 10px 0px',
    '0px 7px 10px 1px',
    '0px 8px 10px 1px',
    '0px 9px 12px 1px',
    '0px 10px 14px 1px',
    '0px 11px 15px 1px',
    '0px 12px 17px 2px',
    '0px 13px 19px 2px',
    '0px 14px 21px 2px',
    '0px 15px 22px 2px',
    '0px 16px 24px 2px',
    '0px 17px 26px 2px',
    '0px 18px 28px 2px',
    '0px 19px 29px 2px',
    '0px 20px 31px 3px',
    '0px 21px 33px 3px',
    '0px 22px 35px 3px',
    '0px 23px 36px 3px',
    '0px 24px 38px 3px',
  ],
  UMBRA: [
    '0px 0px 0px 0px',
    '0px 2px 1px -1px',
    '0px 3px 1px -2px',
    '0px 3px 3px -2px',
    '0px 2px 4px -1px',
    '0px 3px 5px -1px',
    '0px 3px 5px -1px',
    '0px 4px 5px -2px',
    '0px 5px 5px -3px',
    '0px 5px 6px -3px',
    '0px 6px 6px -3px',
    '0px 6px 7px -4px',
    '0px 7px 8px -4px',
    '0px 7px 8px -4px',
    '0px 7px 9px -4px',
    '0px 8px 9px -5px',
    '0px 8px 10px -5px',
    '0px 8px 11px -5px',
    '0px 9px 11px -5px',
    '0px 9px 12px -6px',
    '0px 10px 13px -6px',
    '0px 10px 13px -6px',
    '0px 10px 14px -6px',
    '0px 11px 14px -7px',
    '0px 11px 15px -7px',
  ],
}

const BLACK = {
  AMBIENT: 'hsla(215, 9%, 15%, 12%)',
  PENUMBRA: 'hsla(215, 9%, 15%, 14%)',
  UMBRA: 'hsla(215, 9%, 15%, 20%)',
}

const clampLevel = R.clamp(1, 25)

const dropSpread = R.pipe(R.split(' '), R.dropLast(1), R.join(' '))

export function box(level = 1) {
  level = clampLevel(level)

  const umbra = SHADOWS.UMBRA[level]
  const penumbra = SHADOWS.PENUMBRA[level]
  const ambient = SHADOWS.AMBIENT[level]

  return css`
  box-shadow:
    ${ umbra } ${ BLACK.UMBRA },
    ${ penumbra } ${ BLACK.PENUMBRA },
    ${ ambient } ${ BLACK.AMBIENT };
  `
}

box.animation = animationCSS({
  duration: AnimationDuration.Elevation,
  properties: 'box-shadow',
})

export function text(level = 1) {
  level = clampLevel(level)

  const umbra = dropSpread(SHADOWS.UMBRA[level])
  const penumbra = dropSpread(SHADOWS.PENUMBRA[level])
  const ambient = dropSpread(SHADOWS.AMBIENT[level])

  return css`
  text-shadow:
    ${ umbra } ${ BLACK.UMBRA },
    ${ penumbra } ${ BLACK.PENUMBRA },
    ${ ambient } ${ BLACK.AMBIENT };
    `
}

text.animation = animationCSS({
  duration: AnimationDuration.Elevation,
  properties: 'text-shadow',
})
