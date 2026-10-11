#!/usr/bin/env python3
"""Synthetic CPU OCR comparison. Models and generated images stay outside Git."""
import argparse
import hashlib
import json
import platform
import resource
import statistics
import time
from pathlib import Path


def distance(a, b):
    row = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        nxt = [i]
        for j, y in enumerate(b, 1):
            nxt.append(min(nxt[-1] + 1, row[j] + 1, row[j - 1] + (x != y)))
        row = nxt
    return row[-1]


def normalize(s):
    return ''.join(s.split())


def main():
    from PIL import Image, ImageDraw, ImageFont
    import onnxruntime as ort
    from rapidocr import RapidOCR

    parser = argparse.ArgumentParser()
    parser.add_argument('--models', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--candidate', choices=['v5', 'v6'], required=True)
    parser.add_argument('--font', type=Path, required=True)
    parser.add_argument('--threads', type=int, default=2)
    parser.add_argument('--suite', choices=['sparse', 'dense'], default='sparse')
    args = parser.parse_args()
    if not 1 <= args.threads <= 4:
        parser.error('--threads must be between 1 and 4')
    if args.output.exists() and any(args.output.iterdir()):
        parser.error('--output must be a new or empty directory')
    args.output.mkdir(parents=True, exist_ok=True)
    names = ('ch_PP-OCRv5_det_mobile', 'ch_PP-OCRv5_rec_mobile') if args.candidate == 'v5' else ('multi_PP-OCRv6_det_tiny', 'multi_PP-OCRv6_rec_tiny')
    manifest = Path(__file__).resolve().parents[2] / 'docs/research/next-ai-integration-2026-10-11/models.json'
    expected = {m['key']: m for m in json.loads(manifest.read_text())}
    metadata = []
    for name in names:
        path = args.models / (name + '.onnx')
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        assert digest == expected[name]['sha256'], name
        opts = ort.SessionOptions()
        opts.intra_op_num_threads = args.threads
        opts.inter_op_num_threads = 1
        session = ort.InferenceSession(str(path), opts, providers=['CPUExecutionProvider'])
        meta = session.get_modelmeta().custom_metadata_map
        dictionary = meta.get('character', '')
        metadata.append({'name': name, 'sha256': digest, 'providers': session.get_providers(), 'metadata_keys': list(meta), 'dictionary_sha256': hashlib.sha256(dictionary.encode()).hexdigest() if dictionary else None, 'dictionary_lines': len(dictionary.splitlines()) if dictionary else 0, 'inputs': [{'name': x.name, 'shape': x.shape} for x in session.get_inputs()], 'outputs': [{'name': x.name, 'shape': x.shape} for x in session.get_outputs()]})
        del session
    params = {'Global.use_cls': False, 'Global.log_level': 'warning', 'Det.model_path': str(args.models / (names[0] + '.onnx')), 'Rec.model_path': str(args.models / (names[1] + '.onnx')), 'EngineConfig.onnxruntime.intra_op_num_threads': args.threads, 'EngineConfig.onnxruntime.inter_op_num_threads': 1}
    # Explicit paths override bundled/default v6 small models; no network needed.
    engine = RapidOCR(params=params)
    lines = ['今天下午三点开会，记得带电脑。', '项目进度：已完成测试，等待发布。', '余额 128.50 元，订单 20261011。']
    cases = []
    for size in [12, 14, 16]:
        cases.append((f'chinese-{size}', size, lines, (255, 255, 255), (25, 25, 25)))
    cases += [('code-14', 14, ['const total = price * count;', 'if (value === 0) return null;', 'file_name = "report_2026.csv";'], 'white', 'black'), ('mixed-14', 14, ['任务 Task 42：保存 Save，取消 Cancel', 'CPU 2 threads / 内存 Memory 8GB'], 'white', 'black'), ('numbers-14', 14, ['0123456789 128.50 0.01 1,024.00', 'O0 I1 lI B8 S5 Z2'], 'white', 'black'), ('low-contrast-14', 14, lines, (240, 240, 240), (165, 165, 165)), ('dark-14', 14, lines, (30, 30, 30), (225, 225, 225)), ('blank', 14, [], 'white', 'black'), ('shapes', 14, [], 'white', 'black')]
    if args.suite == 'dense':
        dense = [f'{i + 1:02d}. {lines[i % 3]} Task {100 + i} / progress {i * 3}%' for i in range(30)]
        cases = [('dense-light-14', 14, dense, 'white', 'black'), ('dense-dark-14', 14, dense, (30, 30, 30), (225, 225, 225))]
    rows = []
    for name, size, texts, bg, fg in cases:
        image = Image.new('RGB', (1920, 1080) if args.suite == 'dense' else (960, 600), bg)
        draw = ImageDraw.Draw(image)
        font = ImageFont.truetype(str(args.font), size)
        for i, text in enumerate(texts):
            draw.text((32, 30 + i * (32 if args.suite == 'dense' else 38)), text, font=font, fill=fg)
        if name == 'shapes':
            draw.ellipse((90, 100, 220, 230), fill='orange')
            draw.rectangle((310, 150, 450, 290), fill='blue')
            draw.line((70, 450, 650, 350), fill='green', width=8)
        path = args.output / (name + '.png')
        image.save(path)
        timings, outputs = [], []
        for repeat in range(4):
            start = time.perf_counter()
            result = engine(str(path))
            timings.append((time.perf_counter() - start) * 1000)
            outputs.append(list(result.txts or []))
        text = normalize(''.join(outputs[-1]))
        truth = normalize(''.join(texts))
        rows.append({'name': name, 'truth': texts, 'recognized': outputs[-1], 'scores': list(map(float, result.scores)) if result.scores is not None else [], 'boxes': result.boxes.tolist() if result.boxes is not None else [], 'image_sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'first_ms': timings[0], 'warm_ms': timings[1:], 'stable_text': all(x == outputs[-1] for x in outputs), 'edit_distance': distance(truth, text), 'reference_characters': len(truth), 'cer': distance(truth, text) / len(truth) if truth else None, 'false_positive_lines': len(outputs[-1]) if not truth else None})
        print(json.dumps({'case': name, 'candidate': args.candidate, 'warm_ms': round(statistics.median(timings[1:]), 1), 'cer': rows[-1]['cer']}, ensure_ascii=True), flush=True)
    refs = sum(x['reference_characters'] for x in rows)
    report = {'candidate': args.candidate, 'suite': args.suite, 'platform': platform.platform(), 'onnxruntime': ort.__version__, 'threads': args.threads, 'cpu_quota': Path('/sys/fs/cgroup/cpu.max').read_text().strip() if Path('/sys/fs/cgroup/cpu.max').exists() else None, 'font_sha256': hashlib.sha256(args.font.read_bytes()).hexdigest(), 'metadata': metadata, 'cases': rows, 'summary': {'cer': sum(x['edit_distance'] for x in rows if x['reference_characters']) / refs, 'reference_characters': refs, 'warm_median_ms': statistics.median(t for x in rows for t in x['warm_ms']), 'peak_process_mib': resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / (1024 if platform.system() == 'Linux' else 1048576), 'no_text_false_positive_lines': sum(x['false_positive_lines'] or 0 for x in rows)}}
    (args.output / (args.candidate + '-results.json')).write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(report['summary']), flush=True)


if __name__ == '__main__':
    main()
