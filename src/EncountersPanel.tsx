import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import ActionError from "./ActionError";

export type EncounterView = {kind:"ball"|"butterfly"|null;phase:string;right:boolean};
type Status = EncounterView & {enabled:boolean;quiet_companion:boolean};
export default function EncountersPanel() {
  const [status,setStatus]=useState<Status>({enabled:true,quiet_companion:false,kind:null,phase:"quiet",right:true});
  const [error,setError]=useState("");
  const [busy,setBusy]=useState(false);
  useEffect(()=>{
    let active=true;
    const off=listen<EncounterView>("pet:encounter",e=>{if(active) setStatus(v=>({...v,...e.payload}));});
    const offMemory=listen<{encounters_enabled:boolean;quiet_companion:boolean}>("pet:memory",e=>{if(active) setStatus(v=>({...v,enabled:e.payload.encounters_enabled,quiet_companion:e.payload.quiet_companion}));});
    Promise.all([off,offMemory]).then(()=>invoke<Status>("encounter_status")).then(v=>{if(active) setStatus(v);}).catch(e=>{if(active) setError(String(e));});
    return ()=>{active=false;void off.then(fn=>fn()).catch(console.error);void offMemory.then(fn=>fn()).catch(console.error);};
  },[]);
  const toggle=async(enabled:boolean)=>{
    setBusy(true);setError("");
    try {const memory=await invoke<{encounters_enabled:boolean}>("set_encounters",{enabled});setStatus(v=>({...v,enabled:memory.encounters_enabled}));}
    catch(e){setError(String(e));} finally{setBusy(false);}
  };
  return <section className="play-card encounter-card">
    <div className="encounter-heading"><h2>偶遇小惊喜</h2><label><input aria-label="开启偶遇小事件" type="checkbox" checked={status.enabled} disabled={busy} onChange={e=>void toggle(e.target.checked)}/>开启</label></div>
    <p className="small" data-testid="encounter-phase" data-kind={status.kind||"none"} data-phase={status.phase}>{status.quiet_companion?"安静陪伴中，偶遇暂时暂停；原来的偶遇开关会保留。":!status.enabled?"偶遇已关闭，随时可以重新开启":status.kind==="butterfly"?"小蝴蝶来串门啦":status.kind==="ball"?"大熊把球推过来了，想和你玩": "偶尔会来只蝴蝶，或收到大熊的接球邀请"}</p>
    <p className="small">普通办公时也会偶尔出现；专注、睡觉或与大熊互动时暂停。</p>
    {error?<ActionError error={error} message="偶遇设置保存失败，原设置仍保留。请稍后再试。"/>:null}
  </section>;
}

export function Butterfly({event}:{event:EncounterView}) {
  return event.kind==="butterfly" ? <div className={`butterfly ${event.phase}`} style={{left:event.right?214:56}} data-testid="butterfly" aria-hidden="true">
    <svg viewBox="0 0 32 28" width="28" height="25"><g stroke="#77502e" strokeWidth="1.4"><path className="wing left" fill="#ecb663" d="M15 13 C-4 -4 -3 20 12 23 L15 17Z"/><path className="wing right" fill="#edc677" d="M17 13 C36 -4 35 20 20 23 L17 17Z"/><path d="M16 10v13 M16 11l-4 -6 M16 11l4 -6" fill="none"/></g></svg>
  </div> : null;
}
