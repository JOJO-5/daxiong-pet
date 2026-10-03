#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
frames=$(mktemp -d)
trap 'rm -rf "$frames"' EXIT
source_image="$root/assets/animation-source/frisbee-mouth.png"
height=$(identify -format '%h' "$source_image")
convert "$source_image" -alpha extract -threshold 15% -define connected-components:verbose=true -connected-components 8 null: |
 awk -v cell="$((height/4))" '$NF=="gray(255)" && $(NF-1)>5000 {gsub(":", "", $1);split($2,b,"+");split($3,c,",");print int(c[2]/cell),b[2],$1,$2}' | sort -n -k1,1 -k2,2 > "$frames/objects.tsv"
[[ $(wc -l < "$frames/objects.tsv") == 16 ]]
scale=$(awk '{split($4,b,"[x+]");if(b[1]>w)w=b[1];if(b[2]>h)h=b[2]}END{ws=180/w;hs=166/h;printf "%.5f%%",100*(ws<hs?ws:hs)}' "$frames/objects.tsv")
index=0
while read -r row xpos object bounds; do
 convert "$source_image" -alpha extract -threshold 15% -define connected-components:mean-color=true -define "connected-components:keep=$object" -connected-components 8 -morphology Dilate Disk:1 -alpha copy "$frames/mask.png"
 convert "$source_image" "$frames/mask.png" -compose DstIn -composite -compose Over -trim +repage -filter point -resize "$scale" -gravity south -background none -extent 192x203 -gravity north -extent 192x208 "$frames/frame-$(printf '%02d' "$index").png"
 index=$((index+1))
done < "$frames/objects.tsv"
# The rows preserve read order: the first eight face right, the next eight face left.
convert "$frames/frame-00.png" "$frames/frame-01.png" "$frames/frame-02.png" "$frames/frame-03.png" "$frames/frame-04.png" "$frames/frame-05.png" "$frames/frame-06.png" "$frames/frame-07.png" +append "$frames/right.png"
convert "$frames/frame-08.png" "$frames/frame-09.png" "$frames/frame-10.png" "$frames/frame-11.png" "$frames/frame-12.png" "$frames/frame-13.png" "$frames/frame-14.png" "$frames/frame-15.png" +append "$frames/left.png"
convert "$frames/right.png" "$frames/left.png" -append -define webp:lossless=true "$root/assets/animation-source/frisbee-packed.webp"
convert "$root/public/spritesheet-extended.webp" -crop 1536x4368+0+0 +repage "$frames/original-21.png"
convert "$frames/original-21.png" "$root/assets/animation-source/frisbee-packed.webp" -append -define webp:lossless=true "$root/public/spritesheet-extended.webp"
