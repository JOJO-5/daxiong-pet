#!/usr/bin/env python3
"""Supplemental template/language/JSON checks, not part of the 100-case score.
Run against an already started loopback llama-server; output synthetic data only.
"""
import argparse,json,time,urllib.request
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--out',required=True);args=p.parse_args()
base='http://127.0.0.1:18891'
def call(path,body=None):
 req=urllib.request.Request(base+path,data=json.dumps(body).encode() if body is not None else None,headers={'Content-Type':'application/json'})
 return json.load(urllib.request.urlopen(req,timeout=45))
props=call('/props')
cases=[
 ('short_zh','你是大熊，一只桌面狗狗。用一句中文陪主人聊天。','今天工作好累。',None),
 ('short_en','You are Daxiong, a friendly desktop dog. Reply briefly.','I am tired after work.',None),
 ('action_en','Return only a JSON object with key action. Allowed values: ball, frisbee, tug, come, stop, none.','Let us play frisbee.',None),
 ('memory_en','Extract only an explicit preferred user name. Return only a JSON array. Each object has exactly two fields: key (always user_name) and value (the preferred name).','Please call me Xiao Wang from now on.',None),
 ('action_zh_fewshot','识别玩具，输出JSON。例：玩球 -> {"action":"ball"}；玩飞盘 -> {"action":"frisbee"}；拔河 -> {"action":"tug"}。','一起玩飞盘',None),
 ('action_zh_schema','识别主人想玩的游戏，只输出action，ball代表球，frisbee代表飞盘，tug代表拔河，come代表过来，stop代表停止，none代表无。','一起玩飞盘',{'type':'json_schema','json_schema':{'name':'action','strict':True,'schema':{'type':'object','properties':{'action':{'type':'string','enum':['ball','frisbee','tug','come','stop','none']}},'required':['action'],'additionalProperties':False}}}),
]
memory_format={'type':'json_schema','json_schema':{'name':'memory','strict':True,'schema':{'type':'array','items':{'type':'object','properties':{'key':{'type':'string','enum':['user_name','speech_preference']},'value':{'type':'string'}},'required':['key','value'],'additionalProperties':False}}}}
memory_system='只提取用户本人明确表达的长期称呼或说话偏好。别人的偏好、引用、临时状态不记。称呼key为user_name，说话偏好key为speech_preference。没有明确个人偏好输出[]。示例：以后叫我小王 -> [{"key":"user_name","value":"小王"}]；朋友喜欢叫小王 -> []。'
cases.extend([
 ('memory_zh_fewshot',memory_system,'以后请叫我小王。',None),
 ('memory_zh_schema',memory_system,'以后请叫我小王。',memory_format),
 ('memory_thirdparty_schema',memory_system,'我的朋友喜欢别人叫他小王。',memory_format),
])
results=[]
for name,system,user,fmt in cases:
 body={'messages':[{'role':'system','content':system},{'role':'user','content':user}],'temperature':0.1,'top_k':50,'top_p':1.0,'min_p':0.0,'repeat_penalty':1.05,'max_tokens':96,'cache_prompt':False,'seed':20261010,'chat_template_kwargs':{'enable_thinking':False}}
 if fmt:body['response_format']=fmt
 started=time.perf_counter();result=call('/v1/chat/completions',body)
 results.append({'id':name,'request':body,'response':result,'seconds':time.perf_counter()-started})
 print(name,result['choices'][0]['message'].get('content'),flush=True)
record={'model_alias':props['model_alias'],'template':props['chat_template'],'bos_token':props['bos_token'],'applied_template':call('/apply-template',{'messages':[{'role':'system','content':cases[0][1]},{'role':'user','content':cases[0][2]}]}),'cases':results}
Path(args.out).write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n')
