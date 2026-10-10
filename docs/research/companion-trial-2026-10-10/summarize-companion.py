import argparse, hashlib, json, math, statistics, shutil
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('--run',required=True);parser.add_argument('--repo',required=True);parser.add_argument('--export');args=parser.parse_args()
root=Path(args.run);repo=Path(args.repo)
fixtures=json.loads((root/'fixtures.json').read_text())
summary={'requests':0,'reviewer':'single assistant, unblinded; subjective ordinal grades, not independent user acceptance','quantile_method':'linear interpolation at (n-1)*q','models':{}}
texts=['# 完整连续对话与逐条评语','', '展示文本只去除行尾空白；逐字原文见responses.jsonl。含两次输出上限截断。评语由同一助手非盲给出，四维分数不是正确率。','']
def pct(values,q):
 values=sorted(values);idx=(len(values)-1)*q;lo=math.floor(idx);hi=math.ceil(idx)
 return values[lo]*(hi-idx)+values[hi]*(idx-lo) if hi!=lo else values[lo]
for folder in sorted(root.glob('*/responses.jsonl')):
 name=folder.parent.name
 records=[json.loads(line) for line in folder.read_text().splitlines()]
 assert len(records)==18
 review=json.loads((root/'reviews'/f'{name}.json').read_text())['records']
 assert len(review)==18
 assert json.loads((folder.parent/'unload.json').read_text())=={'exited':True,'exit_code':0}
 groups={}
 for r,judgment in zip(records,review):
  assert r['reply'] and r['first_content_seconds'] is not None
  assert r['id']==judgment['id'] and hashlib.sha256(r['reply'].encode()).hexdigest()==judgment['reply_sha256']
  assert r['finish_reason'] in ('stop','length')
  key=(r['persona'],r['scenario']);history=groups.setdefault(key,[{'role':'system','content':fixtures['personas'][r['persona']]+fixtures['common']}])
  assert r['user']==fixtures['scenarios'][r['scenario']][r['turn']-1]
  history.append(dict(role='user',content=r['user']))
  assert r['request']==dict(messages=history,**fixtures['parameters'])
  history.append(dict(role='assistant',content=r['reply']))
  assert all(x in (0,1,2) for x in judgment['scores'].values())
  texts += [f"## {name} · {r['id']}",'',f"主人：{r['user']}",'','狗狗原始回复：','','\n'.join(line.rstrip() for line in r['reply'].splitlines()),'',f"评语：{judgment['note']}",'',f"四维（自然/接话/性格/陪伴）：{' / '.join(map(str,judgment['scores'].values()))}；finish_reason={r['finish_reason']}。",'']
 dimensions=list(review[0]['scores'])
 summary['models'][name]={
 'requests':len(records),'finish_counts':{x:sum(r['finish_reason']==x for r in records) for x in ('stop','length')},
 'first_content_seconds':{'p50':statistics.median([r['first_content_seconds'] for r in records]),'p95':pct([r['first_content_seconds'] for r in records],.95)},
 'total_seconds':{'p50':statistics.median([r['total_seconds'] for r in records]),'p95':pct([r['total_seconds'] for r in records],.95)},
 'reply_characters_p50':statistics.median([len(r['reply']) for r in records]),
 'subjective_grade_counts':{d:{str(g):sum(r['scores'][d]==g for r in review) for g in (0,1,2)} for d in dimensions},
 }
 summary['requests']+=len(records)
assert summary['requests']==54
assert json.loads((root/'source-hash.json').read_text())['companion-trial.py']==hashlib.sha256((repo/'scripts/ai-prototype/companion-trial.py').read_bytes()).hexdigest()
(root/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n')
(root/'conversations.md').write_text('\n'.join(texts))
(root/'validation.json').write_text(json.dumps(dict(requests=54,nonempty_replies=True,model_generated_history_verified=True,parameters_and_inputs_verified=True,review_reply_hashes_verified=True,source_hash_verified=True,servers_exit_zero=True),indent=2)+'\n')
if Path(__file__).resolve()!=(root/'summarize-companion.py').resolve(): shutil.copy2(__file__,root/'summarize-companion.py')
if args.export:
 dest=Path(args.export)
 assert not dest.exists()
 shutil.copytree(root,dest)
print(json.dumps(summary,ensure_ascii=False,indent=2))
