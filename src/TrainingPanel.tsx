import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {useActivity} from "./ActivityContext";
import ActionError from "./ActionError";
type TrainingMemory={training:number[];treat_wait:number;error:string|null};
const CUES=[{id:"come",label:"过来"},{id:"spin",label:"转圈"},{id:"down",label:"趴下"},{id:"stay",label:"等一下"}];
const PHASES:Record<string,string>={off:"先看你，再听你的小指令",attention:"看着你，认真听呢…",performing:"正在做给你看",completed:"做到了！奖励饼干可以帮助记住",blocked:"先结束专注或放开大熊，再练习吧",cancelled:"这次先停下，换个近一点的位置吧"};
export default function TrainingPanel(){
  const {activity,error:loadError}=useActivity();
  const phase=activity.game==="trick"||activity.phase==="blocked"?activity.phase:"off";
  const [memory,setMemory]=useState<TrainingMemory|null>(null);
  const [at,setAt]=useState(Date.now);
  const [now,setNow]=useState(Date.now);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  useEffect(()=>{
    let active=true;let receivedMemory=false;
    const accept=(v:TrainingMemory)=>{if(active){setMemory(v);setAt(Date.now());setNow(Date.now());}};
    const m=listen<TrainingMemory>("pet:memory",e=>{receivedMemory=true;accept(e.payload);});
    m.then(()=>invoke<TrainingMemory>("companion_status")).then(w=>{if(active&&!receivedMemory)accept(w);}).catch(e=>{if(active)setError(String(e));});
    return ()=>{active=false;void m.then(f=>f()).catch(console.error);};
  },[]);
  useEffect(()=>{if(!memory?.treat_wait)return;const timer=window.setInterval(()=>setNow(Date.now()),1000);return ()=>clearInterval(timer);},[memory]);
  const wait=memory?Math.max(0,memory.treat_wait-Math.floor((now-at)/1000)):0;
  const act=async(command:string,args:Record<string,unknown>={})=>{
    setBusy(true);setError("");try{await invoke(command,args);}catch(e){setError(String(e));}finally{setBusy(false);}
  };
  return <section className="play-card" data-testid="training-panel">
    <h2>一起学小指令</h2><p className="small">完成后奖励饼干，练习三次就学会。离线不遗忘。</p>
    <p role="status" data-testid="trick-phase" data-phase={phase}>{PHASES[phase]||phase}</p>
    <div className="trick-actions">{CUES.map((cue,i)=><button key={cue.id} disabled={busy} data-testid={`trick-${cue.id}`} onClick={()=>void act("trick_action",{cue:cue.id})}>{cue.label}<small data-testid={`skill-${cue.id}`}>{(memory?.training[i]||0)>=3?"已学会":`${memory?.training[i]||0}/3`}</small></button>)}</div>
    <button className="primary" disabled={busy||!activity.rewardable||wait>0||!!memory?.error} onClick={()=>void act("reward_trick")}>奖励这次指令</button>
    <button className="quiet" onClick={()=>void act("trick_action",{cue:"stop"})}>结束练习</button>
    {wait>0?<p className="small">饼干还需等 {wait} 秒，仍可以练习。</p>:null}
    {error||loadError?<ActionError error={error||loadError} message="这次练习没能完成，请稍后再试；专注期间先结束专注。"/>:null}
  </section>;
}
