import { css } from 'styled-components'

const breakpoints = {
  lg: '1056px',
  max: '1584px',
  md: '672px',
  sm: '425px',
  xlg: '1312px',
}

type Breakpoint = keyof typeof breakpoints

export const between = (
  start: Breakpoint,
  end: Breakpoint,
  style: RuleSet<object>,
) => css`
  @media (min-width: ${ breakpoints[start] }) and (max-width: ${ breakpoints[end] }) {
    ${ style }
  }
`

export const above = (breakpoint: Breakpoint, style: RuleSet<object>) => css`
  @media (min-width: ${ breakpoints[breakpoint] }) {
    ${ style }
  }
`

export const below = (breakpoint: Breakpoint, style: RuleSet<object>) => css`
  @media (max-width: ${ breakpoints[breakpoint] }) {
    ${ style }
  }
`
