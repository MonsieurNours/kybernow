#!/bin/bash
# Prépare la vidéo de présentation du site (motion design, sans son) à partir du
# master 4K à 60 i/s :
#
#   scripts/site-video.sh MASTER.mp4
#
# → site/assets/video/hls/ : HLS en trois qualités (720p, 1080p, 2160p, 60 i/s,
#   H.264, segments fMP4 de 4 s alignés sur des images clés toutes les 2 s) et
#   presentation.m3u8, la liste maîtresse ; le lecteur (main.js, hls.js) choisit
#   la qualité selon le débit et la taille du lecteur ;
# → site/assets/video/presentation-720p.mp4 : repli sans HLS (MP4 progressif).
# Le son (AAC 160 kbit/s, stéréo) est dans chaque qualité : le site lance la vidéo
# en muet et le visiteur active le son d'un clic. Durée : quelques minutes sur un
# Mac Apple Silicon (libx264, preset slow).
set -euo pipefail
SRC="${1:?master vidéo (MP4 4K 60 i/s)}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/site/assets/video"
command -v ffmpeg >/dev/null || { echo "ffmpeg manque (brew install ffmpeg)" >&2; exit 1; }
rm -rf "$OUT/hls"; mkdir -p "$OUT/hls"
cd "$OUT/hls"
ffmpeg -hide_banner -loglevel error -stats -y -i "$SRC" \
  -filter_complex "[0:v]split=3[a][b][c];[a]scale=-2:720:flags=lanczos[v0];[b]scale=-2:1080:flags=lanczos[v1];[c]scale=-2:2160:flags=lanczos[v2]" \
  -map "[v0]" -map "[v1]" -map "[v2]" -map 0:a:0 -map 0:a:0 -map 0:a:0 -c:a aac -b:a 160k -ac 2 -ar 48000 -c:v libx264 -preset slow -profile:v high -pix_fmt yuv420p -r 60 \
  -g 120 -keyint_min 120 -sc_threshold 0 -x264-params nal-hrd=none \
  -crf:v:0 21 -maxrate:v:0 3500k -bufsize:v:0 7000k -level:v:0 4.2 \
  -crf:v:1 21 -maxrate:v:1 7000k -bufsize:v:1 14000k -level:v:1 4.2 \
  -crf:v:2 21 -maxrate:v:2 16000k -bufsize:v:2 32000k -level:v:2 5.2 \
  -f hls -hls_time 4 -hls_playlist_type vod -hls_segment_type fmp4 -hls_flags independent_segments \
  -hls_fmp4_init_filename init.mp4 -hls_segment_filename "%v/seg-%03d.m4s" -master_pl_name presentation.m3u8 \
  -var_stream_map "v:0,a:0,name:720p v:1,a:1,name:1080p v:2,a:2,name:2160p" "%v/index.m3u8"
ffmpeg -hide_banner -loglevel error -stats -y -i "$SRC" -vf scale=-2:720:flags=lanczos -c:a aac -b:a 160k -ac 2 -ar 48000 \
  -c:v libx264 -preset slow -profile:v high -pix_fmt yuv420p -crf 23 -maxrate 3000k -bufsize 6000k \
  -movflags +faststart "$OUT/presentation-720p.mp4"
du -sh "$OUT/hls"/* "$OUT/presentation-720p.mp4"
