import fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)


export async function GET() {
    const vtt = await fs.readFile(path.resolve(__dirname, '../../../../public/assets/demo/008_alexandra_juhasz.json'), 'utf-8')

    return new Response(vtt, {
        headers: {
            'Content-Type': 'application/json',
        },
    })
}
