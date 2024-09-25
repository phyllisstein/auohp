import {updateTranscript} from 'app/neo4j'
import type {NextRequest} from 'next/server'
import fs from 'node:fs/promises'
import path from 'node:path'
import {fileURLToPath} from 'node:url'


const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)


export async function GET() {
  const jsonString = await fs.readFile(path.resolve(__dirname, '../../../../public/assets/demo/025_lei_chou.captions.json'), 'utf-8')
  const json = JSON.parse(jsonString)

  return Response.json(json, {
    headers: {
      'Content-Type': 'application/json',
    },
  })
}

export async function PUT(request: NextRequest) {
  let json = await request.json()

  json[0].children = json[0].children.map(segment => {
    const ret = {
      ...segment,
      transcription: segment.children[0].text,
    }
    return ret
  })

  try {
    await updateTranscript(json)
    return Response.json(json, {status: 201})
  } catch (err) {
    return new Response(err.message, {status: 500})
  }
}
