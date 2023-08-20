import { S3Client, S3 } from '@aws-sdk/client-s3'
import { Upload } from '@aws-sdk/lib-storage'
import { useCallback, useEffect, useRef } from 'react'

export default function UploadPage () {
  const s3Ref = useRef(
    new S3({
      region: 'us-east-1',
      credentials: {
        accessKeyId: '***REMOVED***',
        secretAccessKey: '***REMOVED***',
      },
    }),
  )

  useEffect(() => {
    async function upload () {
      const s3 = new S3({
        region: 'us-east-1',
        credentials: {
          accessKeyId: '***REMOVED***',
          secretAccessKey: '***REMOVED***',
        },
      })

      const multipartParams = {
        Bucket: 'act-up-oral-history-resilient-reserve-4710',
        Key: 'test.mp4',
      }

      const videoFetcher = await fetch('/big_buck_bunny_720p_surround.mp4')
      const video = await videoFetcher.blob()

      const upload = await s3.createMultipartUpload(multipartParams)
      const partNumber = 1143
      const partParams = {
        Bucket: 'act-up-oral-history-resilient-reserve-4710',
        Key: 'test.mp4',
        PartNumber: partNumber,
        UploadId: upload.UploadId,
        Body: video,
      }

      const part = await s3.uploadPart(partParams)
      console.log(part)
      const completeParams = {
        Bucket: 'act-up-oral-history-resilient-reserve-4710',
        Key: 'test.mp4',
        MultipartUpload: {
          Parts: [
            {
              ETag: part.ETag,
              PartNumber: partNumber,
            },
          ],
        },
        UploadId: upload.UploadId,
      }

      await s3.completeMultipartUpload(completeParams)
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
