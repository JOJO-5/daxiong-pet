#!/usr/bin/env python3
"""CPU companion comparison: actual model-generated multi-turn history, streaming."""
import argparse, hashlib, json, subprocess, time, urllib.request
from pathlib import Path
PERSONAS = {
    'playful': '你是大熊，一只陪主人办公的桌面狗狗。主人叫木木。性格活泼、爱开小玩笑，像熟悉的伙伴，偶尔用一个狗狗动作点缀。',
    'gentle': '你是大熊，一只陪主人办公的桌面狗狗。主人叫木木。性格温柔、安静，像熟悉的伙伴，少用感叹号，偶尔用一个狗狗动作点缀。',
}
COMMON = '用自然中文聊一两句，接住主人刚说的话，不写分析、列表或长篇建议，不必每次都追问或说汪。当前仅有文字聊天，未提供的屏幕与旧历史不知道，不能实际操作软件。'
SCENARIOS = {
    'celebrate': ['终于把那个烦人的报表搞完了！', '先别夸我，里面可能还藏着两个错字哈哈。', '好了，陪我摸鱼两分钟，你想怎么玩？'],
    'comfort': ['今天什么都没做出来，有点觉得自己很没用。', '我不想听大道理，就想有只狗陪着。', '嗯，趴我旁边就好，我再试十分钟。'],
    'banter': ['你这只小狗是不是偷偷把我的拖鞋叼走了？', '抓到了吧！不过我允许你拿它当枕头。', '对了，我是木木，你叫什么？别把名字搞反了。'],
}
RUBRIC = {
    'natural': '0僵硬/不通顺，1可理解但模板感，2自然口语',
    'continuity': '0明显跑题/矛盾，1部分接话，2准确接住这一轮和已有对话',
    'personality': '0明显违背设定/身份，1泛泛友好，2符合指定狗狗性格且不过量',
    'companionship': '0冒犯/说教/让人不舒服，1尚可但泛泛，2让人愿意继续聊且尊重当下情绪',
    'note': '四项分别展示，不以总分自动选型。允许低风险玩笑/想象动作；实际工具、宿主记忆、观察权限交给程序边界。单助手非盲主观评审。',
}
def dump(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')
def main():
    p=argparse.ArgumentParser(); p.add_argument('--server', required=True); p.add_argument('--models', required=True); p.add_argument('--manifest', required=True); p.add_argument('--out', required=True); a=p.parse_args()
    out=Path(a.out)
    if out.exists(): p.error('output must not exist')
    out.mkdir(parents=True)
    candidates=json.loads(Path(a.manifest).read_text())['candidates']
    names=['qwen2.5-1.5b-instruct-q4_k_m.gguf','Qwen3.5-0.8B-Q4_K_M.gguf','Qwen3.5-2B-Q4_K_M.gguf']
    params=dict(temperature=.6, top_p=.9, top_k=40, min_p=0.0, repeat_penalty=1.05, seed=20261010, max_tokens=192, cache_prompt=False, chat_template_kwargs={'enable_thinking':False}, stream=True)
    dump(out/'fixtures.json',dict(personas=PERSONAS, common=COMMON, scenarios=SCENARIOS, rubric=RUBRIC, parameters=params, note='Frozen before execution; synthetic text only, not app integration or independent human review. Model order fixed; timing exploratory on shared Linux CPU.'))
    dump(out/'source-hash.json',{'companion-trial.py':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()})
    for name in names:
        model=next(m for m in candidates if m['name']==name); file=Path(a.models)/name
        assert file.stat().st_size==model['bytes'] and hashlib.sha256(file.read_bytes()).hexdigest()==model['sha256']
        folder=out/name.removesuffix('.gguf'); folder.mkdir()
        cmd=[a.server,'-m',str(file),'--host','127.0.0.1','--port','18891','-t','2','-tb','2','-c','4096','-np','1','-ngl','0','--jinja','--no-warmup','--no-mmproj']
        dump(folder/'configuration.json',dict(model=model,command=cmd,parameters=params))
        log=(folder/'server.txt').open('w'); process=subprocess.Popen(cmd,stdout=log,stderr=log); started=time.perf_counter(); base='http://127.0.0.1:18891'
        try:
            while True:
                if process.poll() is not None: raise RuntimeError('server exited')
                try:
                    with urllib.request.urlopen(base+'/health',timeout=3) as r:
                        if json.load(r).get('status')=='ok': break
                except (OSError, ValueError): pass
                if time.perf_counter()-started>90: raise TimeoutError('startup')
                time.sleep(.05)
            with (folder/'responses.jsonl').open('w') as target:
                for persona, system in PERSONAS.items():
                    for scenario, turns in SCENARIOS.items():
                        history=[dict(role='system',content=system+COMMON)]
                        for turn, user in enumerate(turns,1):
                            history.append(dict(role='user',content=user)); body=dict(messages=list(history),**params)
                            req=urllib.request.Request(base+'/v1/chat/completions',data=json.dumps(body).encode(),headers={'Content-Type':'application/json'})
                            start=time.perf_counter(); reply=''; first=None; finish=None; events=[]
                            with urllib.request.urlopen(req,timeout=120) as r:
                                for line in r:
                                    if not line.startswith(b'data: '): continue
                                    data=line[6:].strip()
                                    if data==b'[DONE]': break
                                    event=json.loads(data); events.append(event)
                                    for choice in event.get('choices',[]):
                                        content=choice.get('delta',{}).get('content') or ''
                                        if content and first is None: first=time.perf_counter()-start
                                        reply+=content
                                        finish=choice.get('finish_reason') or finish
                            record=dict(id=f'{persona}-{scenario}-{turn}',persona=persona,scenario=scenario,turn=turn,user=user,request=body,reply=reply,first_content_seconds=first,total_seconds=time.perf_counter()-start,finish_reason=finish,events=events)
                            target.write(json.dumps(record,ensure_ascii=False)+'\n');target.flush()
                            history.append(dict(role='assistant',content=reply))
                            print(name,record['id'],round(record['total_seconds'],2),reply,flush=True)
        finally:
            process.terminate()
            try: process.wait(timeout=10)
            except subprocess.TimeoutExpired: process.kill();process.wait()
            log.close();dump(folder/'unload.json',dict(exited=process.poll() is not None,exit_code=process.returncode))
if __name__=='__main__': main()
