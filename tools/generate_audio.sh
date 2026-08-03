#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
audio_dir="$project_root/public/assets/audio"
mkdir -p "$audio_dir"

ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "aevalsrc=0.14*sin(2*PI*(620+950*t)*t)*exp(-10*t):s=48000:d=0.22" \
  -ac 1 -c:a pcm_s16le "$audio_dir/chirp.wav"

ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "aevalsrc=0.16*sin(2*PI*(360-230*t)*t)*exp(-8*t)+0.035*sin(2*PI*95*t)*exp(-12*t):s=48000:d=0.28" \
  -ac 1 -c:a pcm_s16le "$audio_dir/tumble.wav"
