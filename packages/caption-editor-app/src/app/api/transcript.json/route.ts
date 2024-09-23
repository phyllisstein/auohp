import fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)


export async function GET() {
    const jsonString = await fs.readFile(path.resolve(__dirname, '../../../../public/assets/demo/025_lei_chou.captions.json'), 'utf-8')
    let json = JSON.parse(jsonString)
    json = [{
        type: 'transcript',
        children: json,
    }]

    return Response.json(json, {
        headers: {
            'Content-Type': 'application/json',
        },
    })
}

