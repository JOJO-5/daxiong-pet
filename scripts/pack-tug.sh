#!/usr/bin/env bash
# Pack the image-generated, original-identity tug poses; never repaint old rows.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
frames=$(mktemp -d)
trap 'rm -rf "$frames"' EXIT
source_image="$root/assets/animation-source/tug-mouth.png"
height=$(identify -format '%h' "$source_image")
convert "$source_image" -alpha extract -threshold 15% -define connected-components:verbose=true -connected-components 8 null: |
 awk -v cell="$((height/2))" '$NF=="srgb(255,255,255)" || $NF=="gray(255)" {if($(NF-1)>5000){gsub(":", "", $1);split($2,b,"+");split($3,c,",");print int(c[2]/cell),b[2],$1,$2}}' | sort -n -k1,1 -k2,2 > "$frames/objects.tsv"
[[ $(wc -l < "$frames/objects.tsv") == 8 ]]
scale=$(awk '{split($4,b,"[x+]");if(b[1]>w)w=b[1];if(b[2]>h)h=b[2]}END{ws=184/w;hs=194/h;printf "%.5f%%",100*(ws<hs?ws:hs)}' "$frames/objects.tsv")
index=0
while read -r row xpos object bounds; do
 convert "$source_image" -alpha extract -threshold 15% -define connected-components:mean-color=true -define "connected-components:keep=$object" -connected-components 8 -morphology Dilate Disk:1 -alpha copy "$frames/mask.png"
 convert "$source_image" "$frames/mask.png" -compose DstIn -composite -compose Over -trim +repage -filter point -resize "$scale" -gravity south -background none -extent 192x203 -gravity north -extent 192x208 "$frames/frame-$index.png"
 index=$((index+1))
done < "$frames/objects.tsv"
convert "$frames/frame-0.png" "$frames/frame-1.png" "$frames/frame-2.png" "$frames/frame-3.png" "$frames/frame-0.png" "$frames/frame-1.png" "$frames/frame-2.png" "$frames/frame-3.png" +append "$frames/right.png"
convert "$frames/frame-4.png" "$frames/frame-5.png" "$frames/frame-6.png" "$frames/frame-7.png" "$frames/frame-4.png" "$frames/frame-5.png" "$frames/frame-6.png" "$frames/frame-7.png" +append "$frames/left.png"
convert "$frames/right.png" "$frames/left.png" -append -background black -alpha background -define webp:lossless=true "$root/assets/animation-source/tug-packed.webp"
convert "$root/public/spritesheet-extended.webp" -crop 1536x4784+0+0 +repage "$frames/original-23.png"
convert "$frames/original-23.png" "$root/assets/animation-source/tug-packed.webp" -append -background black -alpha background -define webp:lossless=true "$root/public/spritesheet-extended.webp"
# Read the cut tip in each frame so the independent rope meets its baked bite.
python3 "$root/scripts/tug-anchors.py" "$frames" "$root/assets/animation-source"

# Append touch poses after rebuilding the original 25 rows.
if [[ -f "$root/assets/animation-source/belly-rub-source.png" ]]; then
 bash "$root/scripts/pack-petting.sh"
fi
