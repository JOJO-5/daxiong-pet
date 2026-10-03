#!/usr/bin/env bash
set -euo pipefail
# Isolate complete generated dogs instead of cutting tails at approximate grid lines.
root=$(cd "$(dirname "$0")/.." && pwd)
frames=$(mktemp -d)
trap 'rm -rf "$frames"' EXIT
source_image="$root/assets/animation-source/eat-treat.png"
height=$(identify -format '%h' "$source_image")
convert "$source_image" -alpha extract -threshold 15% -define connected-components:verbose=true -connected-components 8 null: |
  awk -v mid="$((height/2))" '$NF=="gray(255)" && $(NF-1)>5000 {gsub(":", "", $1);split($2,b,"+");print (b[3]>=mid?1:0),b[2],$1,$2}' |
  sort -n -k1,1 -k2,2 > "$frames/objects.tsv"
[[ $(wc -l < "$frames/objects.tsv") == 8 ]]
col=0
while read -r row xpos object bounds; do
  convert "$source_image" -alpha extract -threshold 15% -define connected-components:mean-color=true -define "connected-components:keep=$object" -connected-components 8 -morphology Dilate Disk:1 -alpha copy "$frames/mask.png"
  convert "$source_image" "$frames/mask.png" -compose DstIn -composite -compose Over -trim +repage -filter point -resize x197 -gravity south -background none -extent 192x203 -gravity north -extent 192x208 "$frames/frame-$col.png"
  col=$((col+1))
done < "$frames/objects.tsv"
convert "$frames/frame-"*.png +append -define webp:lossless=true "$root/assets/animation-source/eat-packed.webp"
convert "$root/public/spritesheet-extended.webp" -crop 1536x4160+0+0 +repage "$frames/original-20.png"
convert "$frames/original-20.png" "$root/assets/animation-source/eat-packed.webp" -append -define webp:lossless=true "$root/public/spritesheet-extended.webp"
