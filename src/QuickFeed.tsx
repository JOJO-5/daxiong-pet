import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
type Memory={treat_wait:number;error:string|null};
export default function QuickFeed({onReviewMemory,interrupting=false}:{onReviewMemory:()=>void;interrupting?:boolean}){
 const [snapshot,setSnapshot]=useState<{memory:Memory;at:number}|null>(null);const [now,setNow]=useState(Date.now);const [busy,setBusy]=useState(false);const [error,setError]=useState("");
 useEffect(()=>{let active=true;let received=false;const accept=(memory:Memory)=>{if(active){setSnapshot({memory,at:Date.now()});setNow(Date.now());}};
  const off=listen<Memory>("pet:memory",e=>{received=true;accept(e.payload);});off.then(()=>invoke<Memory>("companion_status")).then(v=>{if(!received)accept(v);}).catch(e=>{if(active)setError(String(e));});return()=>{active=false;void off.then(f=>f()).catch(console.error);};},[]);
 useEffect(()=>{if(!snapshot?.memory.treat_wait)return;const timer=window.setInterval(()=>{const value=Date.now();setNow(value);if(value-snapshot.at>=snapshot.memory.treat_wait*1000)clearInterval(timer);},1000);return()=>clearInterval(timer);},[snapshot]);
 const wait=snapshot?Math.max(0,snapshot.memory.treat_wait-Math.floor((now-snapshot.at)/1000)):0;
 const feed=async()=>{setBusy(true);setError("");try{setSnapshot({memory:await invoke<Memory>("feed_treat"),at:Date.now()});setNow(Date.now());}catch(e){setError(String(e));}finally{setBusy(false);}};
 return <div className="quick-feed"><button data-testid="quick-feed" className={`${interrupting?"":"primary "}treat-button`} disabled={busy||!snapshot||wait>0||!!snapshot.memory.error} onClick={()=>void feed()}>{wait>0?`下块饼干 · ${wait}s`:interrupting?"结束互动，喂块饼干":"喂一块饼干"}</button>{snapshot?.memory.error?<><p className="panel-error" role="alert">{snapshot.memory.error}</p><button onClick={onReviewMemory}>查看记忆与恢复</button></>:null}{error?<p className="panel-error" role="alert">{error}</p>:null}</div>;
}
