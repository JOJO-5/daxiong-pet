#!/usr/bin/env python3
"""Encode existing, unchanged atlas pixels as review GIFs, using native timing.

GIFs are inspection artifacts, not replacements for engine-controlled sprites.
Requires Pillow, like audit-pose-scale.py. Does not alter any production asset.
"""
import argparse
import csv
import hashlib
import json
import re
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
args.out.mkdir(parents=True, exist_ok=True)
sheet_path = ROOT / 'public/spritesheet-extended.webp'
sheet = Image.open(sheet_path).convert('RGBA')
atlas = (ROOT / 'src-tauri/src/atlas.rs').read_text()
calibration = list(csv.DictReader((ROOT / 'assets/animation-source/pose-scales.csv').open()))
tracks = ['D_IDLE', 'D_LOCOMOTION', 'D_LOCOMOTION', 'D_WAVE', 'D_JUMP', 'D_FAIL',
          'D_WAIT', 'D_RUN6', 'D_REVIEW', None, None, 'D_HAPPY', 'D_SLEEP', 'D_WAKE',
          'D_SHAKE', 'D_AFFECTION', 'D_LOCOMOTION', 'D_LOCOMOTION', 'D_DROP', 'D_DROP',
          'D_EAT', 'D_LOCOMOTION', 'D_LOCOMOTION', 'D_TUG', 'D_TUG', 'D_BELLY_RUB', 'D_BELLY_ROLL']
names = ['idle', 'run-right', 'run-left', 'wave', 'jump', 'failed', 'waiting', 'busy',
         'review', 'gaze-a-scan', 'gaze-b-scan', 'happy', 'sleep', 'wake', 'shake',
         'affection', 'carry-right', 'carry-left', 'drop-right', 'drop-left', 'eat',
         'disc-right', 'disc-left', 'tug-right', 'tug-left', 'belly-rub', 'belly-roll']
records = []
for row, (track, name) in enumerate(zip(tracks, names)):
    if track:
        match = re.search(r'const ' + track + r': &\[u16\] = &\[([^\]]+)\];', atlas)
        if not match:
            raise ValueError(f'Missing native track {track}')
        text = match[1]
        durations = ([int(text.split(';')[0])] * int(text.split(';')[1]) if ';' in text
                     else [int(x) for x in text.split(',') if x.strip()])
    else:
        durations = [180] * 8  # Angular contact sheet only; native gaze is static.
    frames = []
    for col in range(len(durations)):
        tile = sheet.crop((col * 192, row * 208, (col + 1) * 192, (row + 1) * 208))
        scale = next((float(p['scale']) for p in calibration if int(p['row']) == row
                      and int(p['first_col']) <= col <= int(p['last_col'])), 1.0)
        stage = Image.new('RGBA', (300, 240), (220, 225, 230, 255))
        tile = tile.resize((round(144 * scale), round(156 * scale)), Image.Resampling.NEAREST)
        stage.alpha_composite(tile, (round(150 - 72 * scale), round(236.25 - 152.25 * scale)))
        frames.append(stage.convert('RGB'))
    filename = f'{row:02d}-{name}.gif'
    # GIF stores centiseconds. Round cumulative boundaries, not each frame, so
    # native 275ms tug frames do not lose 20ms on every loop.
    gif_durations = []
    elapsed = previous = 0
    for duration in durations:
        elapsed += duration
        boundary = ((elapsed + 5) // 10) * 10
        gif_durations.append(boundary - previous)
        previous = boundary
    frames[0].save(args.out / filename, save_all=True, append_images=frames[1:],
                   duration=gif_durations, loop=0, optimize=False, disposal=2)
    with Image.open(args.out / filename) as encoded:
        encoded_durations = []
        for i in range(encoded.n_frames):
            encoded.seek(i)
            encoded_durations.append(encoded.info['duration'])
    assert sum(encoded_durations) == sum(gif_durations), filename
    records.append({'row': row, 'file': filename, 'native_durations_ms': durations if track else None,
                    'encoded_frames': len(encoded_durations), 'encoded_durations_ms': encoded_durations,
                    'scope': 'native atlas track; state machines may select subsets or override cadence' if track else 'angle scan only, not native gaze animation'})
(args.out / 'manifest.json').write_text(json.dumps({
    'source_sha256': hashlib.sha256(sheet_path.read_bytes()).hexdigest(),
    'scope': 'Existing pixels, nearest-neighbour projected geometry and GIF palette; not a native render or anatomy certification. GIF boundaries round to 10ms without accumulating cycle drift; identical frames may coalesce.',
    'actions': records}, ensure_ascii=False, indent=2))
print(f'Exported {len(records)} review GIFs; all encoded cycle durations verified.')
