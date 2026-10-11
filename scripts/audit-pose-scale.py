#!/usr/bin/env python3
"""Read-only atlas audit. Requires Pillow; never modifies production artwork.

Alpha bounds/area are diagnostics, not a substitute for checking head, torso,
perspective, anatomy and prop occlusion against the original character.
"""
import argparse
import csv
import hashlib
import json
from pathlib import Path
from PIL import Image

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
original_path = root / 'public/spritesheet.webp'
sheet_path = root / 'public/spritesheet-extended.webp'
original = Image.open(original_path).convert('RGBA')
sheet = Image.open(sheet_path).convert('RGBA')
assert original.size == (1536, 2288) and sheet.size == (1536, 5616)
top = sheet.crop((0, 0, 1536, 2288))
for before, after in zip(original.get_flattened_data(), top.get_flattened_data()):
    assert before[3] == after[3] and (not before[3] or before == after), 'original pixel changed'

with (root / 'assets/animation-source/pose-scales.csv').open() as f:
    calibration = [{k: float(v) if k == 'scale' else int(v) for k, v in row.items()}
                   for row in csv.DictReader(f)]
assert all(11 <= p['row'] < 27 and 0 <= p['first_col'] <= p['last_col'] < 8
           and 1 <= p['scale'] <= 1.5 for p in calibration)

rows = []
for row in range(27):
    frames = []
    for col in range(8):
        tile = sheet.crop((col * 192, row * 208, (col + 1) * 192, (row + 1) * 208))
        alpha = tile.getchannel('A').point(lambda n: 255 if n >= 38 else 0)
        bounds = alpha.getbbox()
        if bounds is None:  # Unused cells in the original tracks are transparent.
            continue
        scale = next((p['scale'] for p in calibration
                      if p['row'] == row and p['first_col'] <= col <= p['last_col']), 1.0)
        area = sum(bool(p) for p in alpha.get_flattened_data())
        # Exactly the native/CSS ground-centered mapping at logical DPI=1.
        x1, y1, x2, y2 = bounds
        display_bounds = [150 + (x1 * .75 - 72) * scale,
                          236.25 + (y1 * .75 - 152.25) * scale,
                          150 + (x2 * .75 - 72) * scale,
                          236.25 + (y2 * .75 - 152.25) * scale]
        assert 0 <= display_bounds[0] < display_bounds[2] <= 300
        assert 0 <= display_bounds[1] < display_bounds[3] <= 240
        frames.append({'col': col, 'alpha_bounds': bounds, 'alpha_area': area,
                       'display_scale': scale, 'display_bounds_logical': display_bounds,
                       'projected_alpha_area_logical': area * (.75 * scale) ** 2})
    rows.append({'row': row, 'frames': frames})

report = {'scope': '27 rows, all nonempty cells, alpha threshold 38/255; props included in area',
          'original_11_rows_pixels_preserved': True,
          'source_sha256': {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                            for p in [original_path, sheet_path]},
          'calibration': calibration, 'rows': rows,
          'note': 'Projected area is geometry, not a rendered-pixel count or an anatomy/identity score.'}
args.out.parent.mkdir(parents=True, exist_ok=True)
args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2))
print(f"Verified 27 rows / {sum(len(row['frames']) for row in rows)} nonempty frames; original pixels preserved; displayed silhouettes inside 300×240.")
