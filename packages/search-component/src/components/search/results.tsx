import {usePortal} from './use-portal'
import ReactDOM from 'react-dom'
import type {ReactPortal, PropsWithChildren} from 'react'
import {ResultsContainer} from './results-styles'

export interface ResultsProps {
  left?: number
  right?: number
  width?: number
  height?: number
  top?: number
  bottom?: number
}

export function Results({
  left = 0,
  right = 0,
  width = 0,
  height = 0,
  top = 0,
  bottom = 0,
  children,
}: PropsWithChildren<ResultsProps>): ReactPortal {
  const portal = usePortal()

  if (!portal) {
    return null
  }

  top = bottom ?? height + top

  return ReactDOM.createPortal(
    <ResultsContainer style={{top, left, width}}>
      {children}
    </ResultsContainer>,
    portal,
  )
}
