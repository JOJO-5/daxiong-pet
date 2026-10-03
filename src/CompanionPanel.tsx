import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type MemoryView = { nickname:string; affection:number; stage:string; treats:number; fetches:number; pats:number; treat_wait:number; error:string|null };
export default function CompanionPanel() {
  const [snapshot,setSnapshot] = useState<{data:MemoryView;at:number}|null>(null);
  const [nickname,setNickname] = useState("");
  const [error,setError] = useState("");
  const [busy,setBusy] = useState(false);
  const [now,setNow] = useState(Date.now);
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
  const act=async(command:string,args:Record<string,unknown>={})=>{
    setBusy(true);setError("");
    try { const data=await invoke<MemoryView>(command,args);setSnapshot({data,at:Date.now()});setNow(Date.now()); }
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
      <button className="primary treat-button" disabled={busy||wait>0||!!memory.error} onClick={()=>void act("feed_treat")}>{wait>0?`下块饼干 · ${wait}s`:"喂一块饼干"}</button>
      <p className="small">记在这台电脑上。暂时离开，感情也不会变淡。</p>
      {memory.error ? <><p className="panel-error" role="alert">{memory.error}。恢复会备份旧文件，并重新开始记录。</p><button disabled={busy} onClick={()=>void act("restore_memory")}>备份旧记忆并重新开始</button></> : null}
    </> : <p>正在读取记忆…</p>}
    {error ? <p className="panel-error" role="alert">{error}</p> : null}
  </section>;
}
