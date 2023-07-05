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

export function Transcript ({ children }: TranscriptProps) {
  return (
    <Container>
      { children }
    </Container>
  )
}
