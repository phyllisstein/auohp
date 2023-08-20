import {
  PutObjectCommand,
  S3Client,
  S3,
  CreateMultipartUploadCommand,
} from '@aws-sdk/client-s3'
import { Upload } from '@aws-sdk/lib-storage'
import { useCallback, useEffect, useRef } from 'react'

export default function UploadPage () {
  const s3Ref = useRef(
    new S3Client({
      region: 'us-east-1',
      credentials: {
        accessKeyId: '***REMOVED***',
        secretAccessKey: '***REMOVED***',
      },
    }),
  )

  useEffect(() => {
    async function upload () {
      const s3 = s3Ref.current

      const video = await fetch('/big_buck_bunny_720p_surround.mp4')
      const blob = await video.blob()

      const command = new PutObjectCommand({
        Bucket: 'act-up-oral-history-resilient-reserve-4710',
        Key: 'test.mp4',
        Body: blob,
      })

      const response = await s3.send(command)
      console.log(response)
    }

    void upload()
  })

  return (
    <div className='spectrum spectrum--medium spectrum--dark'>
      <div className='spectrum-Body spectrum-Body--sizeM'>
        <div className='spectrum-Heading spectrum-Heading--sizeXXL'>Upload</div>
        <div className='spectrum-Body spectrum-Body--sizeM'>
          Upload your video to get started.
          <input
            multiple
            webkitdirectory=''
            id='picker'
            type='file'
            name='picker' />
          <output id='output' />
        </div>
      </div>
    </div>
  )
}
