import fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)


export async function GET() {
    let json = await fs.readFile(path.resolve(__dirname, '../../../../public/assets/demo/025_lei_chou.captions.json'), 'utf-8')
    json = JSON.parse(json)
    json = json.sort((a, b) => b.startTime - a.startTime)
    json = JSON.stringify(json)

    return new Response(json, {
        headers: {
            'Content-Type': 'application/json',
        },
    })
}
