#!/usr/bin/env bash
set -euo pipefail

if [[ $# != 2 ]]; then
    echo 'usage: bash scripts/video-proof.sh ANNEALER NEW_OUTPUT_DIRECTORY' >&2
    exit 2
fi
binary=$(realpath "$1")
output=$2
for tool in ffmpeg ffprobe jq timeout; do
    command -v "$tool" > /dev/null
done
mkdir -- "$output"
output=$(realpath "$output")
ffmpeg -version > "$output/ffmpeg-version.txt"
ffprobe -version > "$output/ffprobe-version.txt"
timeout --kill-after=10s 30m "$binary" export "$output/frames" \
    --frames 300 --fps 30 > "$output/export.log" 2>&1
for ((frame=0; frame<300; frame++)); do
    printf -v name 'frame-%05d.png' "$frame"
    test -s "$output/frames/$name"
done
shopt -s nullglob
images=("$output"/frames/*.png)
[[ ${#images[@]} == 300 ]]
awk -F, '
    NR == 1 { if ($0 != "frame,seconds,avatar_x,avatar_y,avatar_z,created,deleted") exit 1; next }
    { delta = $2 - (NR-2)/30; if ($1 != NR-2 || delta < -0.000001 || delta > 0.000001) exit 1 }
    END { if (NR != 301) exit 1 }
' "$output/frames/frames.csv"
timeout --kill-after=10s 10m ffmpeg -hide_banner -nostdin -n \
    -threads 2 -filter_threads 2 -framerate 30 -start_number 0 \
    -i "$output/frames/frame-%05d.png" -map 0:v:0 -an \
    -c:v libx264 -threads 2 -preset medium -crf 18 -pix_fmt yuv420p \
    -fps_mode passthrough -movflags +faststart "$output/annealer.mp4" \
    > "$output/encode.log" 2>&1
timeout --kill-after=10s 2m ffprobe -v error -threads 2 -count_frames \
    -show_streams -show_format -of json "$output/annealer.mp4" > "$output/probe.json"
jq -e '
    (.streams | length) == 1 and
    (.streams[0] | .codec_name == "h264" and .pix_fmt == "yuv420p" and
        .width == 1280 and .height == 720 and .avg_frame_rate == "30/1" and
        .nb_read_frames == "300") and
    ((.format.duration | tonumber) == 10)
' "$output/probe.json" > /dev/null
timeout --kill-after=10s 2m ffmpeg -hide_banner -nostdin -v error -xerror \
    -threads 2 -i "$output/annealer.mp4" -map 0:v:0 -f null - \
    > "$output/decode.log" 2>&1
cp -- "$output/frames/frames.csv" "$output/frames.csv"
echo "Verified 300 decoded frames, 1280x720, 30 fps, 10 seconds: $output/annealer.mp4"
echo "After visual inspection, remove $output/frames to reclaim the PNG intermediates."
