#!/usr/bin/env bash
set -euo pipefail
# Isolate generated frames, then pack them without changing the original v2 rows.
root=$(cd "$(dirname "$0")/.." && pwd)
frames=$(mktemp -d)
trap 'rm -rf "$frames"' EXIT
source_image="$root/assets/animation-source/daxiong-actions.png"
rows=${1:-5}
# Fixed scale within each action; nearest-neighbor preserves the source pixel clusters.
scales=(100 100 97 94 97)
[[ "$rows" == 3 || "$rows" == 5 ]] || { echo 'rows must be 3 or 5' >&2; exit 1; }
while read -r row col object geometry; do
  ((row < rows)) || continue
  magick "$source_image" -alpha extract -threshold 15% \
    -define connected-components:mean-color=true -define "connected-components:keep=$object" \
    -connected-components 8 -morphology Dilate Disk:1 -alpha copy "$frames/mask.png"
  magick "$source_image" "$frames/mask.png" -compose DstIn -composite -compose Over \
    -crop "$geometry" +repage -filter point -resize "${scales[$row]}%" -gravity south -background none -extent 192x203 \
    -gravity north -extent 192x208 "$frames/$row-$col.png"
done < "$root/assets/animation-source/frames.tsv"
files=("$root/public/spritesheet.webp")
for ((row=0;row<rows;row++)); do
  magick "$frames/$row-"*.png +append "$frames/row-$row.png"
  files+=("$frames/row-$row.png")
done
magick "${files[@]}" -append -define webp:lossless=true "$root/public/spritesheet-extended.webp"

# Re-append dedicated carry cycles when rebuilding all built-in rows.
if [[ "$rows" == 5 && -f "$root/assets/animation-source/carry-mouth.png" ]]; then
  bash "$root/scripts/pack-carry.sh"
fi
