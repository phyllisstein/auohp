import styled from 'styled-components'

const CaptionWrapper = styled.div`
  padding: 12px;

  background-color: var(--spectrum-background-base-color);

  min-inline-size: min-content;
`

export function CaptionLine ({ attributes, children, element }) {
  return (
    <CaptionWrapper { ...attributes } className='spectrum-InLineAlert'>
      <div className='spectrum-InLineAlert-content' style={{ margin: 0 }}>
        Lorem ipsum dolor sit amet consectetur adipisicing elit. Quisquam
      </div>
    </CaptionWrapper>
  )
}
