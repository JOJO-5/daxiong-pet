import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import {ActivityProvider,useActivity} from "./ActivityContext";
import SearchPanel, {HiddenTreat} from "./SearchPanel";
import TrainingPanel from "./TrainingPanel";
import TugRope from "./TugRope";
import { type TugView } from "./tug-view";
import ToyDisc from "./ToyDisc";
import QuickFeed from "./QuickFeed";
import PreferencesPanel from "./PreferencesPanel";
import CompanionPanel from "./CompanionPanel";
import EncountersPanel, { type EncounterView } from "./EncountersPanel";
import {fetchControls, interactionActive} from "./interaction-controls";

type PlayView = { tug:TugView|null; tug_rounds:number; phase: string; catches: number; streak:number; style:string; toy:string; last_catch:string };
const PHASES: Record<string,string> = { off: "准备好陪你玩", catching:"跃起接住飞盘啦！", ready: "拖动桌面上的球，松手抛出", held: "松手，大熊就来追", chasing: "追球中…", returning: "叼回来啦！", teasing: "来追我呀！靠近大熊或点放下球", releasing: "把球放在你脚边", rolling: "把球推给你啦，抓住再扔吧！", returned: "抓起脚边的球，再扔一次吧！" };

export default function PlayPanel(){return <ActivityProvider><PlayContent/></ActivityProvider>;}
function PlayContent() {
  const {activity}=useActivity();
  const [moreOpen,setMoreOpen]=useState(()=>new URLSearchParams(location.search).has("preferences"));
  const details=useRef<HTMLDetailsElement>(null);
  const [play,setPlay] = useState<PlayView>({tug:null,tug_rounds:0,phase:"off",catches:0,streak:0,style:"normal",toy:"ball",last_catch:"none"});
  const [error,setError] = useState("");
  const [busy,setBusy] = useState(false);
  const actionPending=useRef(false);
  const [notice,setNotice] = useState("");
  const [game,setGame]=useState<"fetch"|"frisbee"|"tug"|"tricks"|"snack">("fetch");
  useEffect(()=>{
    if(activity.game==="trick")setGame("tricks");else if(activity.game==="snack")setGame("snack");
  },[activity.game,activity.id]);
  useEffect(()=>{
    if(new URLSearchParams(location.search).has("preferences"))requestAnimationFrame(()=>details.current?.scrollIntoView({block:"start"}));
    const off=listen("pet:preferences",()=>{setMoreOpen(true);requestAnimationFrame(()=>details.current?.scrollIntoView({block:"start"}));});
    const key=(event:KeyboardEvent)=>{if(event.key==="Escape"&&!event.defaultPrevented&&!(event.target instanceof HTMLSelectElement)){event.preventDefault();void invoke("close_playground").catch(e=>setError(String(e)));}};
    window.addEventListener("keydown",key);return()=>{window.removeEventListener("keydown",key);void off.then(f=>f()).catch(console.error);};
  },[]);
  useEffect(() => {
    let active = true;let received=false;
    const off = listen<PlayView>("pet:play", e => { received=true;if(active) setPlay(e.payload); });
    off.then(() => invoke<PlayView>("play_status")).then(v => { if(active&&!received) {setPlay(v);if(v.phase!=="off")setGame(v.toy==="rope"?"tug":v.toy==="frisbee"?"frisbee":"fetch");} }).catch(e => { if(active) setError(String(e)); })
      .finally(()=>{if(active) void invoke("playground_ready").catch(e=>setError(String(e)));});
    const route=listen<string>("pet:game",e=>{if(active&&["fetch","frisbee","tug","tricks","snack"].includes(e.payload))setGame(e.payload as typeof game);});
    return () => { active=false;void route.then(f=>f()).catch(console.error); void off.then(fn=>fn()).catch(console.error); };
  },[]);
  const phase=game==="fetch"||game==="frisbee"?(play.toy===(game==="frisbee"?"frisbee":"ball")?play.phase:"off"):play.phase;
  const controls=fetchControls(phase,game==="frisbee");
  const act = async (action:string) => {
    if(actionPending.current)return false;
    actionPending.current=true;setBusy(true);setError("");setNotice("");
    try { await invoke("play_action",{action});return true; } catch(e) { setError(String(e));return false; }
    finally {actionPending.current=false;setBusy(false);}
  };
  return <main className="play-panel">
    <div className="panel-heading">
    <svg className="panel-icon" aria-hidden="true" viewBox="0 0 32 32" width="32" height="32"><g fill="#567b42"><ellipse cx="16" cy="23" rx="9" ry="6"/><ellipse cx="5" cy="13" rx="3" ry="4"/><ellipse cx="12" cy="7" rx="3" ry="4"/><ellipse cx="21" cy="7" rx="3" ry="4"/><ellipse cx="28" cy="13" rx="3" ry="4"/></g></svg>
    <h1>和大熊一起玩</h1><button className="panel-close" aria-label="关闭面板" title="关闭面板（Esc）" onClick={()=>void invoke("close_playground").catch(e=>setError(String(e)))}>×</button></div><p className="subtitle">挑个玩具，陪大熊玩一会儿</p>
    <div className="game-picker" role="group" aria-label="选择游戏">{([{id:"fetch",label:"接球"},{id:"frisbee",label:"飞盘"},{id:"tug",label:"拔河"},{id:"tricks",label:"小指令"},{id:"snack",label:"找零食"}] as const).map(choice=><button key={choice.id} disabled={busy} aria-pressed={game===choice.id} onClick={async()=>{if(game!==choice.id&&await act("cancel")){setGame(choice.id);setNotice(`已结束上一个互动，准备${choice.label}。`);}}}>{choice.label}</button>)}</div>
    {notice?<p className="small" role="status">{notice}</p>:null}
    {game==="fetch" || game==="frisbee" ? <section className="play-card">
      <h2>{game==="frisbee"?"飞盘时间":"接球时间"}</h2><p role="status" data-testid="play-phase" data-phase={phase}>{game==="frisbee"?({off:"拿出飞盘，或者直接扔一个",ready:"拖住桌面飞盘，甩动后松手",held:"松手，飞盘就会滑翔出去",chasing:"追着滑翔的飞盘跑…",catching:"跃起接住飞盘啦！",returning:"咬住飞盘跑回来啦",releasing:"松口，放到脚边",returned:"抓起脚边的飞盘，可以再扔一次"} as Record<string,string>)[phase]||phase:PHASES[phase]||phase}</p>
      <div className="play-actions">{controls.canShow?<button data-testid="play-show" disabled={busy} onClick={()=>void act(game==="frisbee"?"show_frisbee":"show")}>{game==="frisbee"?"拿出飞盘":"拿出球"}</button>:null}<button className="primary" data-testid="play-throw" disabled={busy||!controls.canThrow} onClick={()=>void act(game==="frisbee"?"throw_frisbee":"throw")}>{controls.throwLabel}</button></div>
      {phase==="teasing" ? <button onClick={()=>void act("drop")}>放下球</button> : null}
      {game==="fetch" && phase==="returned" && play.toy==="ball" ? <button onClick={()=>void act("roll")}>推回给我</button> : null}
      <p className="small" data-testid="fetch-style">{game==="frisbee"?"陪你一起扔飞盘":play.style==="near"?"近近的，慢悠悠捡回来":play.style==="far"?"扔得好远，兴奋追球！":"陪你一起接球"} · 连续 {play.streak} 次</p>
      {game==="fetch"?<PlayfulSwitch />:<p className="small">{play.last_catch==="air"?"这次在空中接到了！":play.last_catch==="ground"?"落地也没关系，捡回来再扔吧":"飞盘会沿浅弧线滑翔，落地后也能捡回"}</p>}
      <button className="quiet" onClick={()=>void act("cancel")}>收起玩具</button>
      <p className="small">本次接回 <strong data-testid="catches">{play.catches}</strong> 次</p>
    </section> : game==="tug" ? <section className="play-card" data-testid="tug-panel">
      <h2>一起拔河</h2>
      <p role="status" data-testid="tug-phase" data-phase={play.toy==="rope"?play.phase:"off"}>
        {play.toy!=="rope"||play.phase==="off"?"拿出绳子，大熊陪你拉一拉":play.phase==="tugging"?"大熊也在使劲！轻拉两秒，再松手歇歇":play.phase==="tug_done"?"拉得好开心！歇一下就能再来一轮":"按住桌面上的橙色绳环，向外轻轻拉"}
      </p>
      <div className="play-actions">{play.toy!=="rope"||play.phase==="off"?<button className="primary" disabled={busy} onClick={()=>void act("start_tug")}>拿出绳子</button>:null}<button disabled={busy} onClick={()=>void act("cancel")}>收起绳子</button></div>
      <p className="small" data-testid="tug-rounds">本次一起玩了 {play.tug_rounds} 轮</p>
      <p className="hint">不用拼手速，也没有输赢。大熊会松劲再拉，松开鼠标就能休息。</p>
    </section> : game==="tricks" ? <TrainingPanel /> : <SearchPanel />}
    <QuickFeed interrupting={interactionActive(play.phase,activity.phase)} onReviewMemory={()=>{setMoreOpen(true);requestAnimationFrame(()=>details.current?.scrollIntoView({block:"start"}));}}/>
    <details className="more-settings" ref={details} open={moreOpen} onToggle={e=>{if(e.currentTarget.open!==moreOpen)setMoreOpen(e.currentTarget.open);}}><summary>更多：记忆与偏好</summary><CompanionPanel hideFeed/><EncountersPanel/><PreferencesPanel/></details>
    <p className="hint">右键大熊可快捷喂食和扔玩具。<br/>拖动大熊或开始专注会收起玩具，Esc 收起面板。</p>
    {error && <p className="panel-error" role="alert">{error}</p>}
  </main>;
}

export function Toy() {
  const [rope,setRope]=useState<TugView|null>(null);
  const [rolling,setRolling]=useState(false);
  const [snack,setSnack]=useState(false);
  const [disc,setDisc]=useState(false);
  const [flying,setFlying]=useState(false);
  useEffect(()=>{
    const off=listen<EncounterView>("pet:encounter",e=>setRolling(e.payload.kind==="ball"&&e.payload.phase==="pushing"));
    let active=true;let received=false;let receivedPlay=false;
    const acceptPlay=(v:PlayView)=>{if(active){setRope(v.tug);setDisc(v.toy==="frisbee");setFlying(v.phase==="chasing");}};
    const play=listen<PlayView>("pet:play",e=>{receivedPlay=true;acceptPlay(e.payload);});
    play.then(()=>invoke<PlayView>("play_status")).then(v=>{if(!receivedPlay)acceptPlay(v);}).catch(console.error);
    const activity=listen<{treat:unknown}>("pet:activity",e=>{received=true;if(active)setSnack(e.payload.treat!=null);});
    activity.then(()=>invoke<{treat:unknown}>("trick_status")).then(v=>{if(active&&!received)setSnack(v.treat!=null);}).catch(console.error);
    return ()=>{active=false;void off.then(fn=>fn()).catch(console.error);void activity.then(fn=>fn()).catch(console.error);void play.then(fn=>fn()).catch(console.error);};
  },[]);
  if(snack)return <HiddenTreat/>;
  if(rope)return <TugRope rope={rope}/>;
  if(disc)return <div className="toy-stage"><div className={`toy-frisbee${flying?" flying":""}`} title="拖动飞盘，甩动后松手" data-testid="toy-frisbee"><ToyDisc/></div></div>;
  return <div className="toy-stage"><div className={`toy-ball${rolling?" rolling":""}`} title="拖动后松手抛球" data-testid="toy-ball"/></div>;
}

function PlayfulSwitch() {
  const [enabled,setEnabled]=useState(true);
  const [error,setError]=useState("");
  useEffect(()=>{
    let active=true;
    invoke<{playful_fetch:boolean}>("companion_status").then(v=>{if(active)setEnabled(v.playful_fetch);}).catch(e=>{if(active)setError(String(e));});
    return ()=>{active=false;};
  },[]);
  return <><label className="small playful-switch"><input type="checkbox" checked={enabled} onChange={async e=>{
    try {const v=await invoke<{playful_fetch:boolean}>("set_playful_fetch",{enabled:e.target.checked});setEnabled(v.playful_fetch);setError("");}
    catch(err){setError(String(err));}
  }}/> 偶尔叼球逗你追</label>{error ? <p role="alert">{error}</p> : null}</>;
}
