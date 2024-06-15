import { css as styledCSS } from 'styled-components'

// import { PropertiesHyphen } from 'csstype' // FIXME: Not sure why this is breaking.

// Q.v. <https://github.com/angular/components/blob/master/src/material/core/animation/animation.ts#L20-L22>.
export enum AnimationDuration {
  Complex = 375,
  Elevation = 280,
  Entering = 225,
  Exiting = 195,
  SwiftIn = 300,
  SwiftInOut = 500,
  SwiftLinear = 80,
  SwiftOut = 400,
}

// Q.v. <https://github.com/angular/components/blob/master/src/material/core/animation/animation.ts#L11-L14>.
// Q.v. <https://github.com/angular/components/blob/master/src/material/core/style/_variables.scss#L24-L44>.
export enum AnimationCurve {
  MaterialAcceleration = 'cubic-bezier(0.4, 0, 1, 1)',
  MaterialDeceleration = 'cubic-bezier(0, 0, 0.2, 1)',
  MaterialSharp = 'cubic-bezier(0.4, 0, 0.6, 1)',
  MaterialStandard = 'cubic-bezier(0.4, 0, 0.2, 1)',
  SwiftIn = 'cubic-bezier(0.55, 0, 0.55, 0.2)',
  SwiftInOut = 'cubic-bezier(0.55, 0, 0.55, 0.2)',
  SwiftLinear = 'linear',
  SwiftOut = 'cubic-bezier(0.25, 0.8, 0.25, 1)',
}

export interface CSSParams {
  curve?: AnimationCurve
  duration?: AnimationDuration
  properties?: string[] | string
}

export function css({
  curve = AnimationCurve.MaterialStandard,
  duration = AnimationDuration.Entering,
  properties = ['all'],
}: CSSParams = {}) {
  const property = Array.from(properties).join(',')

  return styledCSS`
        transition: ${ property } ${ duration }ms ${ curve };
    `
}
