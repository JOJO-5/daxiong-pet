import json,time,threading,resource,traceback,os
from pathlib import Path
import litert_lm
import argparse
parser=argparse.ArgumentParser(description='Independent LiteRT CPU lifecycle probe; work directory is <model-and-venv-directory>/round2.')
parser.add_argument('--work',type=Path,required=True)
root=parser.parse_args().work.resolve()
root.mkdir(parents=True,exist_ok=True)
result={'runtime':'litert-lm 0.18.0','model':'Qwen3.5-0.8B INT8','backend':'CPU','threads':2,'context':1024,'thinking':False,'gpu_tested':False,'checks':[]}
def save():
    result['peak_rss_kib']=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    (root/'lifecycle-result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2))
def check(name,ok):
    result['checks'].append({'name':name,'passed':bool(ok)});save();print(name,ok,flush=True)
engine=None;chat=None
try:
    started=time.monotonic()
    engine=litert_lm.Engine(str(root.parent/'Qwen3.5-0.8B_int8.litertlm'),backend=litert_lm.Backend.CPU(thread_count=2),max_num_tokens=1024)
    result['load_seconds']=round(time.monotonic()-started,3)
    chat=engine.create_conversation(system_message='你是一只友好的桌面狗狗，简短回复。',max_output_tokens=32,thinking_config=litert_lm.ThinkingConfig(enable_thinking=False))
    replies=[]
    for prompt in ['我的昵称是小林，请记住，只回复好的。','刚才我说我的昵称是什么？只回复昵称。']:
        started=time.monotonic();answer=str(chat.send_message(prompt,max_output_tokens=32))
        replies.append({'prompt':prompt,'reply':answer,'seconds':round(time.monotonic()-started,3)})
    result['multi_turn']=replies
    check('two real turns return nonempty content',all(x['reply'].strip() for x in replies))
    check('second turn recalls synthetic nickname', '小林' in replies[1]['reply'])
    chat.close();chat=None
    stream=engine.create_conversation(system_message='按用户要求生成内容。',max_output_tokens=128,thinking_config=litert_lm.ThinkingConfig(enable_thinking=False))
    first=threading.Event();finished=threading.Event();chunks=[];errors=[]
    def generate():
        try:
            for chunk in stream.send_message_async('请依次列出从1到200的整数，不要提前结束。',max_output_tokens=128):
                chunks.append(str(chunk));first.set()
        except Exception as e:errors.append(str(e))
        finally:finished.set()
    worker=threading.Thread(target=generate,daemon=True);worker.start()
    check('stream emits a real chunk before cancellation',first.wait(60))
    before_cancel=time.monotonic();already_done=finished.is_set();stream.cancel_process();returned=finished.wait(5)
    result['cancel']={'already_finished':already_done,'finished_within_5s':returned,'seconds':round(time.monotonic()-before_cancel,3),'chunks':chunks,'errors':errors}
    check('cancel interrupts an active native generation',not already_done and returned)
    if not returned:
        save();os._exit(2)
    worker.join();stream.close()
    chat=engine.create_conversation(system_message='你是桌面狗狗，回复简短。',max_output_tokens=16,thinking_config=litert_lm.ThinkingConfig(enable_thinking=False))
    result['after_cancel_reply']=str(chat.send_message('你好，简短打个招呼。',max_output_tokens=16))
    check('same engine creates a working conversation after cancellation',bool(result['after_cancel_reply'].strip()))
    chat.close();chat=None;engine.close();engine=None
    check('explicit conversation and engine close complete',True)
    result['status']='passed' if all(c['passed'] for c in result['checks']) else 'failed_checks'
except Exception as error:
    result['status']='error';result['error']=str(error);traceback.print_exc()
finally:
    if chat is not None:chat.close()
    if engine is not None:engine.close()
    save();print(json.dumps(result,ensure_ascii=False),flush=True)
if result.get('status')!='passed':raise SystemExit(1)
