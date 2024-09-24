import {type NextRequest, type NextResponse} from 'next/server'

import {connect, disconnect, transcriptSearch, type Neo4jResult} from 'app/neo4j'

export async function GET(
  request: NextRequest,
  response: NextResponse,
) {
  const searchParams = request.nextUrl.searchParams
  const query = searchParams.get('query')
  const index = searchParams.get('index') || 'transcript_search'

  if (!query) {
    return Response.json({}, {status: 204})
  }

  let searchResults: Neo4jResult[] = []
  try {
    await connect()
    searchResults = await transcriptSearch(query, index)
    return Response.json({searchResults})
  } catch {
    return Response.json({searchResults}, {status: 500})
  } finally {
    await disconnect()
  }
}
