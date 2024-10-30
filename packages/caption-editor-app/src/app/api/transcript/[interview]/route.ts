import {getInterviewTranscript} from 'app/neo4j'
import type {NextRequest} from 'next/server'

interface Params {
  params: {
    interview: string
  }
}

export async function GET(request: NextRequest, {params}: Params) {
  const {interview} = await params
  const interviewId = Number.parseInt(interview)

  if (!interviewId) {
    return new Response('Missing interview query parameter', {status: 400})
  }

  const transcript = await getInterviewTranscript(interviewId)

  return Response.json(transcript, {
    headers: {
      'Content-Type': 'application/json',
    },
  })
}
