#!/bin/env fish

# Train a custom Amazon Transcribe language model with text files resembling the
# conversations it will transcribe. This creates an initial corpus from Jim's
# precise and accurate manual transcripts.

# Extract text from PDFs and normalize to UTF-8. Anything but plain Unicode text
# will throw an error. pdftotext is part of `poppler` or `poppler-utils`.
for pdf in *.pdf
    set basename (basename -s .pdf $pdf)
    pdftotext -enc UTF-8 "$pdf" $basename".txt"
    sed -i 's/�//g; s/[^[:print:]\t\n ]//g' $basename".txt"
end

# Upload the text files to S3.
aws s3 sync . s3://$AWS_BUCKET/training/ --exclude "*" --include "*.txt"

# Create the model. $AWS_TRANSCRIBE_ARN is the ARN of an IAM role granting
# Transcribe access to the S3 bucket.
aws transcribe create-language-model \
    --language-code en-US \
    --base-model-name WideBand \
    --model-name auohp \
    --input-data-config S3Uri=s3://$AWS_BUCKET/training/,DataAccessRoleArn=$AWS_TRANSCRIBE_ARN

# Run a transcription job using the model. Produces JSON with structured
# transcription and timing data, as well as a WebVTT file with subtitles.
aws transcribe start-transcription-job \
    --transcription-job-name crimp-(date +%s) \
    --language-code en-US \
    --media MediaFileUri=s3://$AWS_BUCKET/074_douglas_crimp.flac \
    --output-bucket-name $AWS_BUCKET \
    --settings ShowSpeakerLabels=true,MaxSpeakerLabels=2 \
    --model-settings LanguageModelName=auohp \
    --subtitles Formats=vtt,srt,OutputStartIndex=1
