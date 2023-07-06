import styled from 'styled-components'

interface TranscriptProps {
  children?: React.ReactNode
}

const Container = styled.div`
  display: grid;
  grid-gap: 2rem;
  grid-template-rows: 1fr;
  grid-template-columns: 1fr;
`

export function Transcript ({ attributes, children, element }: TranscriptProps) {
  // console.log({ attributes, children, element })
  return (
    <div { ...attributes }>
      <Container>
        { children }
      </Container>
    </div>
  )
}
