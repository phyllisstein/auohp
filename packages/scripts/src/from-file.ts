import {
    StartTranscriptionJobCommand,
    type StartTranscriptionJobCommandInput,
    TranscribeClient,
} from "@aws-sdk/client-transcribe";

const client = new TranscribeClient({
    region: "us-east-1",
});

const BUCKET = "act-up-oral-history-resilient-reserve-4710";

const FILES = [
    "004_gregg_bordowitz",
    "012_mark_harrington",
    "035_larry_kramer",
    "074_douglas_crimp",
    "138_joan_gibbs",
];

const BASE_PARAMS: StartTranscriptionJobCommandInput = {
    JobExecutionSettings: {
    // FIXME: "Please provide data access role for jobs that allow deferred execution."
    // AllowDeferredExecution: true,
    },
    LanguageCode: "en-US",
    Media: {
        MediaFileUri: "",
    },
    MediaFormat: "flac",
    ModelSettings: {
        LanguageModelName: "auohp-1688512598",
    },
    OutputBucketName: BUCKET,

    Settings: {
        MaxSpeakerLabels: 3,
        ShowSpeakerLabels: true,
    },
    Subtitles: {
        Formats: ["vtt", "srt"],
    },

    TranscriptionJobName: "",
};

async function main(fn = "") {
    const start = Date.now();
    console.log("Starting transcription jobs at", fn);

    for await (const file of FILES) {
        const params = {
            ...BASE_PARAMS,
            Media: {
                MediaFileUri: `s3://${ BUCKET }/${ file }.flac`,
            },
            OutputKey: `${ file }.json`,
            TranscriptionJobName: `${ start }-${ file }`,
        };

        console.log("Starting transcription job for", file);

        try {
            const command = new StartTranscriptionJobCommand(params);
            const result = await client.send(command);
            console.log("Transcription job started for", file, result);
        } catch (err) {
            console.error("Error starting transcription job for", file, err);
        }
    }
}

void (await main());
