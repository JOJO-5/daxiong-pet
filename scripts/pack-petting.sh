#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
frames=$(mktemp -d)
trap 'rm -rf "$frames"' EXIT
source_image="$root/assets/animation-source/belly-rub-source.png"
magick "$source_image" -alpha extract -threshold 15% -define connected-components:verbose=true -connected-components 8 null: |
  awk '$NF=="srgb(255,255,255)" && $(NF-1)>5000 {gsub(":", "", $1);split($2,b,"+");print (b[3]>=512?1:0),b[2],$1,$2}' |
  sort -n -k1,1 -k2,2 > "$frames/objects.tsv"
[[ $(wc -l < "$frames/objects.tsv") == 6 ]]
col=0
while read -r row xpos object bounds; do
  magick "$source_image" -alpha extract -threshold 15% -define connected-components:mean-color=true -define "connected-components:keep=$object" -connected-components 8 -morphology Dilate Disk:1 -alpha copy "$frames/mask.png"
  magick "$source_image" "$frames/mask.png" -compose DstIn -composite -compose Over -trim +repage -filter point -resize 36% -gravity south -background none -extent 192x203 -gravity north -extent 192x208 "$frames/pose-$col.png"
  col=$((col+1))
done < "$frames/objects.tsv"
# Keep the canonical standing and crouching frames unchanged.
magick "$root/public/spritesheet.webp" -crop 192x208+0+0 +repage "$frames/standing.png"
magick "$root/public/spritesheet-extended.webp" -crop 192x208+384+2704 +repage "$frames/crouch.png"
magick "$frames/pose-1.png" "$frames/pose-2.png" "$frames/pose-3.png" "$frames/pose-4.png" "$frames/pose-1.png" "$frames/pose-3.png" "$frames/pose-2.png" "$frames/pose-4.png" +append -define webp:lossless=true "$root/assets/animation-source/belly-rub-packed.webp"
magick "$frames/standing.png" "$frames/crouch.png" "$frames/pose-0.png" "$frames/pose-1.png" "$frames/pose-1.png" "$frames/pose-5.png" "$frames/crouch.png" "$frames/standing.png" +append -define webp:lossless=true "$root/assets/animation-source/belly-roll-packed.webp"
magick "$root/public/spritesheet-extended.webp" -crop 1536x5200+0+0 +repage "$frames/existing-25.png"
magick "$frames/existing-25.png" "$root/assets/animation-source/belly-rub-packed.webp" "$root/assets/animation-source/belly-roll-packed.webp" -append -define webp:lossless=true "$root/public/spritesheet-extended.webp"
magick "$frames/standing.png" "$frames/crouch.png" "$frames/pose-0.png" "$frames/pose-1.png" "$frames/pose-2.png" "$frames/pose-3.png" "$frames/pose-4.png" "$frames/pose-5.png" +append -background '#888' -alpha remove "$root/assets/animation-source/petting-identity-review.png"
