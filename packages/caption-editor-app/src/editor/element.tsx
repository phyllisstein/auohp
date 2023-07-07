import { CaptionLine } from './captions'
import { Transcript, TranscriptRow } from './transcript'

export function Element (props) {
  const { attributes, children, element } = props

  switch (element.type) {
  case 'caption-line':
    return (
      <CaptionLine { ...props } />
    )
  case 'transcript':
    return (
      <Transcript { ...props } />
    )
  case 'transcript-row':
    return (
      <TranscriptRow { ...props } />
    )
  default:
    return <p { ...attributes }>{ children }</p>
  }
}
