import {Editor} from 'components/editor'
import {Container, PlayerColumn} from './page.styles'

interface Params {
  params: {
    interview: string
  }
}

export default async function Page({params}: Params) {
  async function fetchTranscript() {
    'use server'

    const {interview} = await params

    return fetch(`http://127.0.0.1:3030/api/transcript/${interview}`)
      .then(async response => {
        if (!response.ok) {
          console.error(response)
          throw new Error('Failed to fetch transcript')
        }
        const json = await response.json()
        return json
      })
      .catch(console.error)
  }

  const transcript = await fetchTranscript()

  return (
    <Container>
      <Editor editorTranscript={transcript} />
      <PlayerColumn>
        <div />
      </PlayerColumn>
    </Container>
  )
}
