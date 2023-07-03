import { CaptionLine } from './caption-line'

export function Element (props) {
  switch (props.element.type) {
  case 'caption-line':
    return (
      <CaptionLine { ...props } />
    )
  default:
    return <p { ...props } />
  }
}
