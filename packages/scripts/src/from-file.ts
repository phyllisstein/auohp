import {
  StartTranscriptionJobCommand,
  type StartTranscriptionJobCommandInput,
  TranscribeClient,
} from '@aws-sdk/client-transcribe'

const client = new TranscribeClient({
  region: 'us-east-1',
})

const BUCKET = 'act-up-oral-history-resilient-reserve-4710'

const FILES = [
  '035-000',
  '035-001',
  '035-002',
  '035-003',
]

const BASE_PARAMS: StartTranscriptionJobCommandInput = {
  TranscriptionJobName: '',
  LanguageCode: 'en-US',
  Settings: {
    MaxSpeakerLabels: 5,
    ShowSpeakerLabels: true,
  },
  JobExecutionSettings: {
    // FIXME: "Please provide data access role for jobs that allow deferred execution."
    // AllowDeferredExecution: true,
  },
  OutputBucketName: BUCKET,

  MediaFormat: 'wav',
  Media: {
    MediaFileUri: '',
  },
  MediaSampleRateHertz: 48000,

  Subtitles: {
    Formats: ['vtt', 'srt'],
  },
}

async function main (fn = '') {
  const start = Date.now()
  console.log('Starting transcription jobs at', fn)

  for await (const file of FILES) {
    const params = {
      ...BASE_PARAMS,
      TranscriptionJobName: `${ start }-${ file }`,
      Media: {
        MediaFileUri: `s3://${ BUCKET }/035/${ file }.wav`,
      },
      OutputKey: `035/${ file }.json`,
    }

    console.log('Starting transcription job for', file)

    try {
      const command = new StartTranscriptionJobCommand(params)
      const result = await client.send(command)
      console.log('Transcription job started for', file, result)
    } catch (err) {
      console.error('Error starting transcription job for', file, err)
    }
  }
}

void await main()
