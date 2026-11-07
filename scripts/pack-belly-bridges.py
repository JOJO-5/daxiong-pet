#!/usr/bin/env python3
"""Pack the two reviewed generated in-betweens; never redraw original pixels.

Requires Pillow. The source is two poses side by side on transparency. Its
standing/lying endpoints and all other atlas rows remain exactly as before.
"""
from pathlib import Path
import os
import tempfile
from PIL import Image

root = Path(__file__).resolve().parents[1]
folder = root / 'assets/animation-source'
source = Image.open(folder / 'belly-roll-bridges-source.png').convert('RGBA')
sheet = Image.open(root / 'public/spritesheet-extended.webp').convert('RGBA')
assert sheet.size == (1536, 5616)
before = sheet.copy()
middle = source.width // 2
poses = []
for index, width in enumerate([160, 182]):
    pose = source.crop((index * middle, 0, (index + 1) * middle, source.height))
    bounds = pose.getchannel('A').point(lambda value: 255 if value >= 38 else 0).getbbox()
    assert bounds is not None
    pose = pose.crop(bounds)
    pose = pose.resize((width, round(pose.height * width / pose.width)), Image.Resampling.NEAREST)
    assert pose.height <= 203
    cell = Image.new('RGBA', (192, 208))
    # Crouch approaches the original frontal head before leaning onto the side.
    cell.alpha_composite(pose, (188 - width if index == 0 else (192 - width) // 2,
                               203 - pose.height))
    poses.append(cell)
for col, pose in [(1, poses[0]), (2, poses[1]), (5, poses[1]), (6, poses[0])]:
    sheet.paste(pose, (col * 192, 26 * 208))
assert sheet.crop((0, 0, 1536, 26 * 208)).tobytes() == before.crop((0, 0, 1536, 26 * 208)).tobytes()
for col in [0, 3, 4, 7]:
    box = (col * 192, 26 * 208, (col + 1) * 192, 27 * 208)
    assert sheet.crop(box).tobytes() == before.crop(box).tobytes()
def save_atomic(image, path):
    fd, temporary = tempfile.mkstemp(suffix='.webp', dir=path.parent)
    os.close(fd)
    try:
        image.save(temporary, lossless=True, exact=True)
        os.replace(temporary, path)
    finally:
        Path(temporary).unlink(missing_ok=True)

save_atomic(sheet.crop((0, 26 * 208, 1536, 27 * 208)), folder / 'belly-roll-packed.webp')
save_atomic(sheet, root / 'public/spritesheet-extended.webp')
print('Packed four bridge cells; original 26 rows and four endpoint cells preserved.')
