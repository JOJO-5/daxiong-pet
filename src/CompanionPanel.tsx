import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import ActionError from "./ActionError";

type MemoryView = { nickname:string; affection:number; stage:string; treats:number; fetches:number; pats:number; treat_wait:number; error:string|null };
export default function CompanionPanel({hideFeed=false}:{hideFeed?:boolean}) {
  const [snapshot,setSnapshot] = useState<{data:MemoryView;at:number}|null>(null);
  const [nickname,setNickname] = useState("");
  const [error,setError] = useState("");
  const [busy,setBusy] = useState(false);
  const [now,setNow] = useState(Date.now);
  const [success,setSuccess]=useState("");
  const [confirming,setConfirming]=useState(false);
  const [recovery,setRecovery]=useState<{directory:string;has_file:boolean}|null>(null);
  useEffect(()=>{
    let active=true;
    let received=false;
    const accept=(data:MemoryView)=>{ if(active) { setSnapshot({data,at:Date.now()}); setNow(Date.now()); } };
    const off=listen<MemoryView>("pet:memory",e=>{ received=true; accept(e.payload); });
    const offError=listen<string>("pet:memory-error",e=>{ if(active) setError(e.payload); });
    Promise.all([off,offError]).then(()=>invoke<MemoryView>("companion_status")).then(data=>{
      if(active) { if(!received) accept(data); setNickname(data.nickname); }
    }).catch(e=>{ if(active) setError(String(e)); });
    return ()=>{ active=false; void off.then(fn=>fn()).catch(console.error); void offError.then(fn=>fn()).catch(console.error); };
  },[]);
  useEffect(()=>{
    if(!snapshot?.data.treat_wait) return;
    const timer=window.setInterval(()=>setNow(Date.now()),1000);
    return ()=>window.clearInterval(timer);
  },[snapshot]);
  const memoryError=snapshot?.data.error;
  useEffect(()=>{
    if(!memoryError){setRecovery(null);setConfirming(false);return;}
    let active=true;
    invoke<{directory:string;has_file:boolean}>("memory_recovery_info").then(v=>{if(active)setRecovery(v);}).catch(e=>{if(active)setError(String(e));});
    return()=>{active=false;};
  },[memoryError]);
  const act=async(command:string,args:Record<string,unknown>={})=>{
    setBusy(true);setError("");setSuccess("");
    try { const data=await invoke<MemoryView>(command,args);setSnapshot({data,at:Date.now()});setNow(Date.now());
      setSuccess(command==="set_nickname"?(data.nickname?`记住啦，以后叫你${data.nickname}。`:"已恢复默认称呼。") : command==="restore_memory"?"旧文件已保留，新的陪伴记忆准备好了。":"");
      if(command==="restore_memory")setConfirming(false);
    }
    catch(e) { setError(String(e)); }
    finally { setBusy(false); }
  };
  const memory=snapshot?.data;
  const wait=memory ? Math.max(0,memory.treat_wait-Math.floor((now-(snapshot?.at||now))/1000)) : 0;
  const milestone=memory ? memory.affection<20?20:memory.affection<100?100:memory.affection<300?300:1000 : 20;
  return <section className="play-card companion-card">
    <h2>我们的陪伴记忆</h2>
    {memory ? <>
      <p className="small"><strong data-testid="bond-stage">{memory.stage}</strong> · 熟悉程度 <span data-testid="affection">{memory.affection}</span>/{milestone}</p>
      <progress aria-label="熟悉程度" value={memory.affection} max={milestone}/>
      <p className="small memory-stats">摸头 <span data-testid="memory-pats">{memory.pats}</span> · 饼干 <span data-testid="memory-treats">{memory.treats}</span> · 接球 <span data-testid="memory-fetches">{memory.fetches}</span></p>
      <label className="nickname-label" htmlFor="nickname">大熊怎么称呼你？</label>
      <div className="nickname-row"><input id="nickname" value={nickname} maxLength={16} placeholder="你的昵称" onChange={e=>setNickname(e.target.value)}/><button disabled={busy||!!memory.error} onClick={()=>void act("set_nickname",{nickname})}>记住昵称</button></div>
      {success?<p className="small" role="status" data-testid="memory-success">{success}</p>:null}
      {hideFeed?null:<button className="primary treat-button" disabled={busy||wait>0||!!memory.error} onClick={()=>void act("feed_treat")}>{wait>0?`下块饼干 · ${wait}s`:"喂一块饼干"}</button>}
      <p className="small">记在这台电脑上。暂时离开，感情也不会变淡。</p>
      {memory.error ? <><ActionError error={memory.error} message="陪伴记忆读取失败，旧文件尚未被覆盖。可以先备份，再重新开始记录。"/>
        {!confirming?<button disabled={busy||!recovery} onClick={()=>setConfirming(true)}>备份旧记忆并重新开始</button>:<div className="recovery-confirm" role="group" aria-label="确认重新开始">
          <p>重新开始会清空昵称、熟悉程度、练习进度和互动记录，并恢复默认陪伴偏好。</p>
          <p className="small">{recovery?.has_file?"旧文件将备份为 companion.backup-*.json，保存在：":"未发现可备份的旧文件；新记忆将保存在："}<br/>{recovery?.directory}</p>
          <button data-testid="confirm-memory-restore" disabled={busy} onClick={()=>void act("restore_memory")}>确认备份并重新开始</button>
          <button disabled={busy} onClick={()=>setConfirming(false)}>保留现有记忆</button>
        </div>}
      </> : null}
    </> : <p>正在读取记忆…</p>}
    {error ? <ActionError key={error} error={error} message="保存或读取失败，现有记忆仍保留。请检查昵称和存储权限后再试。"/> : null}
  </section>;
}
