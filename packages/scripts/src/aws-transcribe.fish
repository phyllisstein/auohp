#!/usr/bin/env fish

# If necessary, create an S3 bucket for training data and transcription output.
if test -z "$AWS_BUCKET"
    set -x AWS_BUCKET auohp-transcribe-(date +%s)
    aws s3 mb s3://$AWS_BUCKET
end

# Train a custom Amazon Transcribe language model with text files resembling the
# conversations it will transcribe. This creates an initial corpus from Jim's
# precise and accurate manual transcripts.
function train-custom-model
    set -l pdf_dir $argv[1]
    pushd $pdf_dir

    # Extract text from PDFs and normalize to UTF-8. Anything but plain Unicode text
    # will throw an error. pdftotext is part of `poppler` or `poppler-utils`.
    for pdf in *.pdf
        set basename (basename -s .pdf $pdf)
        pdftotext -enc UTF-8 "$pdf" "$basename".txt
        sed -i 's/�//g; s/[^[:print:]\t\n ]//g' "$basename".txt
    end

    # Upload text of PDF transcripts to S3.
    aws s3 sync . s3://$AWS_BUCKET/training/ --exclude "*" --include "*.txt"

    # Create an IAM role granting Transcribe access to the S3 bucket.
    if test -z "$AWS_TRANSCRIBE_ARN"
        aws iam create-role \
            --role-name TranscribeS3AccessRole \
            --assume-role-policy-document '{"Version": "2012-10-17", "Statement": [{"Effect": "Allow", "Principal": {"Service": "transcribe.amazonaws.com"}, "Action": "sts:AssumeRole"}]}' >/dev/null 2>&1

        # Grant the IAM role access to the S3 bucket.
        aws iam attach-role-policy \
            --role-name TranscribeS3AccessRole \
            --policy-arn arn:aws:iam::aws:policy/AmazonS3FullAccess

        # Get the ARN of the role.
        set AWS_TRANSCRIBE_ARN (aws iam get-role --role-name TranscribeS3AccessRole --query Role.Arn --output text)
    end

    set MODEL_NAME auohp-pdf-transcripts-(date +%s)

    # Create the model.
    aws transcribe create-language-model \
        --language-code en-US \
        --base-model-name WideBand \
        --model-name $MODEL_NAME \
        --input-data-config S3Uri=s3://$AWS_BUCKET/training/,DataAccessRoleArn=$AWS_TRANSCRIBE_ARN

    echo -e "

    \e[1;5m⌛ Training model...\e[0m

    \e[90mTo check progress, run:\e[0m
    \taws transcribe describe-language-model --model-name" $MODEL_NAME "

    "

    popd
end

# Run a transcription job using the model. Produces JSON with structured
# transcription and timing data, as well as a WebVTT file with TV-dinner
# subtitles.
function run-transcription-job
    set -l video_dir $argv[1]
    pushd $video_dir

    for video in $video_dir/*.mp4
        set -l basename (basename -s .mp4 $video | sed 's/[^a-zA-Z0-9]//g' | string lower)

        ffmpeg -i "$video" -vn -acodec flac -ar 16000 -ac 1 -f flac -y "$basename".flac
        aws s3 cp "$basename".flac s3://$AWS_BUCKET/

        aws transcribe start-transcription-job \
            --transcription-job-name $basename \
            --language-code en-US \
            --media MediaFileUri=s3://$AWS_BUCKET/$basename.flac \
            --output-bucket-name $AWS_BUCKET \
            --settings ShowSpeakerLabels=true,MaxSpeakerLabels=3 \
            --model-settings LanguageModelName=auohp-pdf-transcripts \
            --subtitles Formats=vtt,srt,OutputStartIndex=1
    end

    popd
end

if test (count $argv) -lt 2
    echo "Usage: $argv[0] <function> <path>"
    echo "Functions: train-custom-model, run-transcription-job"
    exit 1
end

set -l function_to_run $argv[1]
set -l file_path $argv[2]

switch $function_to_run
    case "train-custom-model"
        train-custom-model $file_path
    case "run-transcription-job"
        run-transcription-job $file_path
    case '*'
        echo "Invalid function: $function_to_run"
        echo "Usage: $argv[0] <function> <path>"
        echo "Functions: train-custom-model, run-transcription-job"
        exit 1
end
