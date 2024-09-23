import { Editor } from './editor'
import { Container, Video, VideoColumn } from './page-styles'


export default async function Page() {
    async function fetchTranscript() {
        'use server'
        return fetch('http://127.0.0.1:3030/api/transcript.json')
            .then(async response => {
                if (!response.ok) {
                    throw new Error('Failed to fetch transcript')
                }
                const json = await response.json()
                return json
            })
            .catch(console.error)
    }

    async function updateTranscript() {
        'use server'
    }

    const transcript = await fetchTranscript()

    return (
        <Container>
            <Editor editorTranscript={ transcript } />
            <VideoColumn>
                <Video />
            </VideoColumn>
        </Container>
    )
}
