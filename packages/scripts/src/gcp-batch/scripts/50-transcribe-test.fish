#!/usr/bin/env fish

gsutil -m cp gs://auohp/00_larry_kramer_test.wav ~/00_larry_kramer_test.wav
subwhisp transcribe ~/00_larry_kramer_test.wav

set output_files ~/00_larry_kramer_test.json ~/00_larry_kramer_test.captions.json ~/00_larry_kramer_test.vtt
for output_file in $output_files
    if not test -f $output_file
        echo "Transcription did not produce $output_file"
        echo "Check subwhisp output above"
        exit 1
    end
end
