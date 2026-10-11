#!/usr/bin/env python3
"""Native CPU protocol/quality probe with a synthetic PNG, no real screenshot."""
import argparse
import json
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--ocr', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
runtime = args.ocr.resolve()
binary = runtime / ('daxiong-ocr.exe' if __import__('os').name == 'nt' else 'daxiong-ocr')
command = [str(binary), str(runtime / 'ch_PP-OCRv5_det_mobile.onnx'), str(runtime / 'ch_PP-OCRv5_rec_mobile.onnx'), '2']
fixture = root / 'docs/research/ocr-cpu-trial-2026-10-11/v5/chinese-12.png'
started = time.monotonic()
result = subprocess.run(command, input=fixture.read_bytes(), stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, cwd=runtime)
args.out.mkdir(parents=True, exist_ok=True)
(args.out / 'worker-diagnostics.json').write_text(json.dumps({'exit':result.returncode,'stderr':result.stderr.decode(errors='replace')[:1000],'runtime_files':[p.name for p in runtime.iterdir()]},indent=2))
assert result.returncode == 0, f'native OCR worker failed, exit={result.returncode}, stderr={result.stderr.decode(errors="replace")[:300]}'
data = json.loads(result.stdout)
assert data['provider'] == 'cpu' and data['version'] == 1
actual = ''.join(''.join(line['text'] for line in data['lines']).split())
assert actual == '今天下午三点开会，记得带电脑。项目进度：已完成测试，等待发布。余额128.50元，订单20261011。'
assert len(data['lines']) == 3
invalid = subprocess.run(command, input=b'not-a-png', stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, cwd=runtime)
assert invalid.returncode != 0 and not invalid.stdout
args.out.mkdir(parents=True, exist_ok=True)
(args.out / 'ocr-native.json').write_text(json.dumps({'elapsed_seconds': time.monotonic() - started, 'result': data, 'malformed_png_rejected': True}, ensure_ascii=False, indent=2) + '\n')
print('Native CPU OCR, 12px Chinese and malformed-image checks passed')
