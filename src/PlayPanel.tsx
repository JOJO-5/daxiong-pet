import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import CompanionPanel from "./CompanionPanel";
import EncountersPanel, { type EncounterView } from "./EncountersPanel";

type PlayView = { phase: string; catches: number };
const PHASES: Record<string,string> = { off: "准备好陪你玩", ready: "拖动桌面上的球，松手抛出", held: "松手，大熊就来追", chasing: "追球中…", returning: "叼回来啦！", returned: "再来一球？" };

export default function PlayPanel() {
  const [play,setPlay] = useState<PlayView>({phase:"off",catches:0});
  const [error,setError] = useState("");
  useEffect(() => {
    let active = true;
    const off = listen<PlayView>("pet:play", e => { if(active) setPlay(e.payload); });
    off.then(() => invoke<PlayView>("play_status")).then(v => { if(active) setPlay(v); }).catch(e => { if(active) setError(String(e)); })
      .finally(()=>{if(active) void invoke("playground_ready").catch(e=>setError(String(e)));});
    return () => { active=false; void off.then(fn=>fn()).catch(console.error); };
  },[]);
  const act = async (action:string) => {
    setError("");
    try { await invoke("play_action",{action}); } catch(e) { setError(String(e)); }
  };
  return <main className="play-panel">
    <svg className="panel-icon" aria-hidden="true" viewBox="0 0 32 32" width="32" height="32"><g fill="#567b42"><ellipse cx="16" cy="23" rx="9" ry="6"/><ellipse cx="5" cy="13" rx="3" ry="4"/><ellipse cx="12" cy="7" rx="3" ry="4"/><ellipse cx="21" cy="7" rx="3" ry="4"/><ellipse cx="28" cy="13" rx="3" ry="4"/></g></svg>
    <h1>和大熊一起玩</h1><p className="subtitle">一颗球，就能快乐一下午</p>
    <section className="play-card">
      <h2>接球时间</h2><p role="status" data-testid="play-phase" data-phase={play.phase}>{PHASES[play.phase] || play.phase}</p>
      <div className="play-actions"><button onClick={()=>void act("show")}>拿出球</button><button className="primary" onClick={()=>void act("throw")}>抛一球</button></div>
      <button className="quiet" onClick={()=>void act("cancel")}>收起玩具</button>
      <p className="small">本次接球 <strong data-testid="catches">{play.catches}</strong> 次</p>
    </section>
    <CompanionPanel />
    <EncountersPanel />
    <p className="hint">也可以抓住桌面上的球，甩动后松手。<br/>拖动大熊或开始专注会收起玩具。</p>
    {error && <p className="panel-error" role="alert">{error}</p>}
  </main>;
}

export function Toy() {
  const [rolling,setRolling]=useState(false);
  useEffect(()=>{
    const off=listen<EncounterView>("pet:encounter",e=>setRolling(e.payload.kind==="ball"&&e.payload.phase==="pushing"));
    return ()=>{void off.then(fn=>fn()).catch(console.error);};
  },[]);
  return <div className="toy-stage"><div className={`toy-ball${rolling?" rolling":""}`} title="拖动后松手抛球" data-testid="toy-ball"/></div>;
}
