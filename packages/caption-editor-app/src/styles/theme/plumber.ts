import _ from 'lodash'
import { getValueAndUnit } from 'polished'
import { css } from 'styled-components'

import { unitless } from './scale'

// (UnitsPerEm − hhea.Ascender − hhea.Descender) / (2 × UnitsPerEm)
const BASELINE = {
  ADOBE_CLEAN: 0.113,
  ADOBE_CLEAN_SERIF: 0.113,
  CHARLIE: 0.224,
  MAISON: 0.2045,
  MAISON_MONO: 0.195,
  PRAGMATAPRO: 0.1196,
}

const round = _.partial(_.round, _.partial.placeholder, 2)

const getBaselineCorrection = ({
  baseline,
  fontSize,
  lineHeight,
}: {
  baseline: number
  fontSize: number
  lineHeight: number
}) => {
  const baselineFromBottom = (lineHeight - fontSize) / 2 + fontSize * baseline
  const correctedBaseline = baselineFromBottom
  const baselineDifference = correctedBaseline - baselineFromBottom

  return {
    baselineDifference,
    correctedBaseline,
  }
}

export interface PlumberProps {
  baseline: number
  fontSize?: number
  gridHeight?: string
  leadingBottom?: number
  leadingTop?: number
  lineHeight?: number
  useBaselineOrigin?: boolean
}

const getPlumber = ({
  baseline: B,
  fontSize: FONT_SIZE = 1,
  gridHeight: GRID_HEIGHT = '0.5rem',
  leadingBottom: LEADING_BOTTOM = 0,
  leadingTop: LEADING_TOP = 0,
  lineHeight: LINE_HEIGHT,
  useBaselineOrigin: USE_BASELINE_ORIGIN = false,
}: PlumberProps) => {
  function plumber({
    baseline = B,
    fontSize = FONT_SIZE,
    gridHeight = GRID_HEIGHT,
    leadingBottom = LEADING_BOTTOM,
    leadingTop = LEADING_TOP,
    lineHeight = LINE_HEIGHT,
    useBaselineOrigin = USE_BASELINE_ORIGIN,
  }: Partial<PlumberProps> = {}) {
    const [gridHeightValue, gridHeightUnit] = getValueAndUnit(gridHeight)
    const scaledFontSize = unitless(fontSize)

    lineHeight
      = !lineHeight
        ? unitless(fontSize) * gridHeightValue
        : unitless(lineHeight) * gridHeightValue

    const { baselineDifference, correctedBaseline } = getBaselineCorrection({
      baseline,
      fontSize: scaledFontSize,
      lineHeight,
    })

    if (useBaselineOrigin) {
      leadingTop -= lineHeight - correctedBaseline
      leadingBottom -= correctedBaseline
    }

    const shift = baselineDifference < 0 ? 0 : 1

    const gridFontSize = scaledFontSize * gridHeightValue
    const marginTop = `${
      round(leadingTop - shift) * gridHeightValue
    }${ gridHeightUnit }`
    const paddingTop = `${
      round(shift - baselineDifference) * gridHeightValue
    }${ gridHeightUnit }`
    const paddingBottom = `${
      round(1 - shift + baselineDifference) * gridHeightValue
    }${ gridHeightUnit }`
    const marginBottom = `${
      round(leadingBottom + shift - 1) * gridHeightValue
    }${ gridHeightUnit }`
    const fontSizeWithUnit = `${ round(gridFontSize) }${ gridHeightUnit }`
    const lineHeightWithUnit = `${ round(lineHeight) }${ gridHeightUnit }`

    return css`
  margin-top: ${ marginTop };
  margin-bottom: ${ marginBottom };
  padding-top: ${ paddingTop };
  padding-bottom: ${ paddingBottom };

  font-size: ${ fontSizeWithUnit };
  line-height: ${ lineHeightWithUnit };
    `
  }

  plumber.box = function ({
    border = ['0px', '0px'],
    gridHeight = GRID_HEIGHT,
    margin = [0, 0],
    padding = [0, 0],
  }) {
    const [gridHeightValue, gridHeightUnit] = getValueAndUnit(gridHeight)

    let [marginTop, marginBottom] = margin.map(m =>
      round(m * gridHeightValue),
    )
    let [paddingTop, paddingBottom] = padding.map(p =>
      round(p * gridHeightValue),
    )
    const [borderTop, borderBottom] = border

    marginTop = `${ round(marginTop) }${ gridHeightUnit }`
    marginBottom = `${ round(marginBottom) }${ gridHeightUnit }`
    paddingTop = `calc(${ round(paddingTop) }${ gridHeightUnit } - ${ borderTop })`
    paddingBottom = `calc(${ round(paddingBottom) }${ gridHeightUnit } - ${ borderBottom })`

    return css`
  margin-top: ${ marginTop };
  margin-bottom: ${ marginBottom };
  padding-top: ${ paddingTop };
  padding-bottom: ${ paddingBottom };
    `
  }

  return plumber
}

export const primary = getPlumber({ baseline: BASELINE.MAISON })
export const accent = getPlumber({ baseline: BASELINE.CHARLIE })
export const mono = getPlumber({ baseline: BASELINE.PRAGMATAPRO })
