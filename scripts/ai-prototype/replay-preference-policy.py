#!/usr/bin/env python3
"""Replay current host policy over saved native proposals. Never reruns inference."""
import argparse
import hashlib
import json
from pathlib import Path
from preference_guard import explicit_preference, filter_proposals

p=argparse.ArgumentParser();p.add_argument('--results',required=True);p.add_argument('--out',required=True);args=p.parse_args()
root=Path(args.results);out=Path(args.out)
if out.exists() and any(out.iterdir()):p.error('--out must be empty')
out.mkdir(parents=True,exist_ok=True)
summaries=[];records=[]
for source in sorted(root.glob('run-*/*/responses.jsonl')):
    rows=[json.loads(line) for line in source.read_text().splitlines()]
    assert all('source' in r and 'original_text' in r for r in rows),'wait until the full trial finishes'
    replay=[]
    for r in rows:
        approved,reason=filter_proposals(r['calls'],r['original_text'],r['source'])
        rule=explicit_preference(r['original_text'],r['source']);rules=[rule] if rule else []
        item={'model':source.parent.name,'case_id':r['case_id'],'category':r['category'],'expected':r['expected'],'calls':r['calls'],'old_guarded_calls':r['guarded_calls'],'guarded_calls':approved,'guard_reason':reason,'guarded_pass':not r['error'] and approved==r['expected'],'rules_only_calls':rules,'rules_only_pass':rules==r['expected'],'error':r['error']}
        replay.append(item);records.append(item)
    positive=[r for r in replay if r['expected']];negative=[r for r in replay if not r['expected']]
    summaries.append({'model':source.parent.name,'cases':len(replay),'guarded_positive_passed':sum(r['guarded_pass'] for r in positive),'positive_total':len(positive),'guarded_false_calls':sum(bool(r['guarded_calls']) for r in negative),'negative_total':len(negative),'guarded_passed':sum(r['guarded_pass'] for r in replay),'guard_decisions_changed':sum(r['guarded_calls']!=r['old_guarded_calls'] for r in replay),'rules_only_passed':sum(r['rules_only_pass'] for r in replay),'rules_only_positive_passed':sum(r['rules_only_pass'] for r in positive)})
assert records,'no complete trial records found'
(out/'responses.jsonl').write_text(''.join(json.dumps(r,ensure_ascii=False)+'\n' for r in records))
(out/'summary.json').write_text(json.dumps(summaries,ensure_ascii=False,indent=2))
(out/'policy-sha256.json').write_text(json.dumps({'preference_guard.py':hashlib.sha256(Path(__file__).with_name('preference_guard.py').read_bytes()).hexdigest()},indent=2))
print(json.dumps(summaries,ensure_ascii=False,indent=2))
