#!/usr/bin/env bash
set -euo pipefail
# Pack generated biting cycles into two new rows; preserve the first 16 rows.
root=$(cd "$(dirname "$0")/.." && pwd)
frames=$(mktemp -d)
trap 'rm -rf "$frames"' EXIT
for row in 0 1; do
  for col in {0..7}; do
    convert "$root/assets/animation-source/carry-mouth.png" -crop "256x384+$((col*256))+$((row*384))" +repage "$frames/cell.png"
    object=$(convert "$frames/cell.png" -alpha extract -threshold 15% -define connected-components:verbose=true -connected-components 8 null: | awk '$NF=="gray(255)" {gsub(":", "", $1); print $1; exit}')
    [[ -n "$object" ]]
    convert "$frames/cell.png" -alpha extract -threshold 15% -define connected-components:mean-color=true -define "connected-components:keep=$object" -connected-components 8 -morphology Dilate Disk:1 -alpha copy "$frames/mask.png"
    convert "$frames/cell.png" "$frames/mask.png" -compose DstIn -composite -compose Over -trim +repage \
      -filter point -resize 70% -gravity south -background none -extent 192x203 -gravity north -extent 192x208 "$frames/$row-$col.png"
  done
  convert "$frames/$row-"*.png +append "$frames/row-$row.png"
done
convert "$frames/row-0.png" "$frames/row-1.png" -append -define webp:lossless=true "$root/assets/animation-source/carry-packed.webp"
convert "$root/public/spritesheet-extended.webp" -crop 1536x3328+0+0 +repage "$frames/original-16.png"
convert "$frames/original-16.png" "$root/assets/animation-source/carry-packed.webp" -append -define webp:lossless=true "$root/public/spritesheet-extended.webp"

if [[ -f "$root/assets/animation-source/drop-mouth.png" ]]; then
  bash "$root/scripts/pack-drop.sh"
fi
