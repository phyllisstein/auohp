import styled from 'styled-components'

const RowContainer = styled.div`
  display: grid;
  grid-gap: 0.5rem;
  grid-template-rows: 1fr;
  grid-template-columns: 1fr 3fr;
`

export function TranscriptRow ({ attributes, children, element }) {
  return (
    <RowContainer { ...attributes }>
      { children }
    </RowContainer>
  )
}
