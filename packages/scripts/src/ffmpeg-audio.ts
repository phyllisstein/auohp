import { createFFmpeg, fetchFile } from '@ffmpeg/ffmpeg'

const ffmpeg = createFFmpeg({ log: true })
const FILE_NAME = '026'

async function main () {
  await ffmpeg.load()
  ffmpeg.FS('writeFile', `../assets/026/`, await fetchFile('https://act-up-oral-history-resilient-reserve-4710.s3.amazonaws.com/026/026.wav'))
}

await main()
