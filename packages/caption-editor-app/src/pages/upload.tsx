import {
  CompleteMultipartUploadCommand,
  CreateMultipartUploadCommand,
  S3Client,
  UploadPartCommand,
} from '@aws-sdk/client-s3'
import { useCallback, useEffect, useRef } from 'react'

/**
 * TK: Split the file up and upload in chunks.
 *    @see {https://gist.github.com/Arp-G/e808d47f80e49458548bd7b37ebdeeb7}
 */
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

      const createCommand = new CreateMultipartUploadCommand({
        Bucket: 'act-up-oral-history-resilient-reserve-4710',
        Key: 'test.mp4',
      })

      const createResponse = await s3.send(createCommand)
      console.log(`Created upload:`, createResponse)

      const uploadPartCommand = new UploadPartCommand({
        Bucket: 'act-up-oral-history-resilient-reserve-4710',
        Key: 'test.mp4',
        UploadId: createResponse.UploadId,
        PartNumber: 1,
        Body: blob,
      })
      const uploadPartResponse = await s3.send(uploadPartCommand)
      console.log(`Uploaded part:`, uploadPartResponse)

      const completeCommand = new CompleteMultipartUploadCommand({
        Bucket: 'act-up-oral-history-resilient-reserve-4710',
        Key: 'test.mp4',
        MultipartUpload: {
          Parts: [
            {
              ETag: uploadPartResponse.ETag,
              PartNumber: 1,
            },
          ],
        },
        UploadId: createResponse.UploadId,
      })

      console.log(`Complete command:`, completeCommand)
      // const completeCommand = new CompleteMultipartUploadCommand({
      //   Bucket: 'act-up-oral-history-resilient-reserve-4710',
      //   Key: 'test.mp4',
      //   UploadId: createResponse.UploadId,
      //   MultipartUpload: {
      //     Parts: [
      //       {
      //         ETag: uploadPartResponse.ETag,
      //         PartNumber: 1,
      //       },
      //     ],
      //   },
      // })
      const completeResponse = await s3.send(completeCommand)
      if (completeResponse.$metadata.httpStatusCode !== 200) {
        throw new Error('Failed to complete upload')
      } else {
        console.log(`Uploaded file:`, completeResponse)
      }
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
