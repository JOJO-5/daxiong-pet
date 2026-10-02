#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
frames=$(mktemp -d)
trap 'rm -rf "$frames"' EXIT
read -r width height < <(identify -format '%w %h\n' "$root/assets/animation-source/drop-mouth.png")
cw=$((width/4)); ch=$((height/2))
for row in 0 1; do
  for col in {0..3}; do
    convert "$root/assets/animation-source/drop-mouth.png" -crop "${cw}x${ch}+$((col*cw))+$((row*ch))" +repage "$frames/cell.png"
    object=$(convert "$frames/cell.png" -alpha extract -threshold 15% -define connected-components:verbose=true -connected-components 8 null: | awk '$NF=="gray(255)" {gsub(":", "", $1); print $1; exit}')
    [[ -n "$object" ]]
    convert "$frames/cell.png" -alpha extract -threshold 15% -define connected-components:mean-color=true -define "connected-components:keep=$object" -connected-components 8 -morphology Dilate Disk:1 -alpha copy "$frames/mask.png"
    convert "$frames/cell.png" "$frames/mask.png" -compose DstIn -composite -compose Over -trim +repage -filter point -resize 180x158 -gravity south -background none -extent 192x203 -gravity north -extent 192x208 "$frames/$row-$col.png"
  done
  for col in {4..7}; do cp "$frames/$row-3.png" "$frames/$row-$col.png"; done
  convert "$frames/$row-"*.png +append "$frames/row-$row.png"
done
convert "$frames/row-0.png" "$frames/row-1.png" -append -define webp:lossless=true "$root/assets/animation-source/drop-packed.webp"
convert "$root/public/spritesheet-extended.webp" -crop 1536x3744+0+0 +repage "$frames/original-18.png"
convert "$frames/original-18.png" "$root/assets/animation-source/drop-packed.webp" -append -define webp:lossless=true "$root/public/spritesheet-extended.webp"
