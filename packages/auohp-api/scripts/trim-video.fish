#!/usr/bin/env fish

# Usage: trim-random.fish input.mp4 [output.mp4]

set input $argv[1]
set output (test -n "$argv[2]"; and echo $argv[2]; or echo (string replace -r '\.mp4$' '' $input)_trimmed.mp4)

if test -z "$input"
    echo "Usage: trim-random.fish input.mp4 [output.mp4]"
    exit 1
end

# Get total duration in seconds (ffprobe returns a float)
set duration (ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 $input)
set duration_int (math -s0 $duration)

set clip_length 300  # 5 minutes

if test $duration_int -le $clip_length
    echo "Video is shorter than 5 minutes; copying as-is."
    cp $input $output
    exit 0
end

# Pick a random start time, leaving room for the full clip at the end
set max_start (math $duration_int - $clip_length)
set start (random 0 $max_start)

echo "Duration: {$duration_int}s — trimming from {$start}s to "(math $start + $clip_length)"s"

ffmpeg -ss $start -i $input -t $clip_length -c copy $output
