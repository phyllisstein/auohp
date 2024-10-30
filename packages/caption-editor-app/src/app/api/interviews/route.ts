import {listInterviews} from 'app/neo4j'

export async function GET() {
  const interviews = await listInterviews()

  return Response.json(interviews, {
    headers: {
      'Content-Type': 'application/json',
    },
  })
}
