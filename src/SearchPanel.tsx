import {useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {useActivity} from "./ActivityContext";
import ActionError from "./ActionError";
const PHASES:Record<string,string>={off:"藏好一块零食，看看大熊能不能找到",placing:"拖动桌面零食，放好后开始寻找",held:"放在你想藏的位置，再松手",ready:"藏好了，让大熊找一找吧",attention:"认真观察，准备出发",searching:"跑过去找零食…",observing:"停下来看看，会藏在哪儿呢？",found:"找到了！再藏一块吧",cancelled:"这次先停下，换个近一点的位置吧",blocked:"先结束专注或放开大熊，再一起玩吧"};
export default function SearchPanel(){
  const {activity,error:loadError}=useActivity();
  const [difficulty,setDifficulty]=useState("easy");
  const [error,setError]=useState("");const [busy,setBusy]=useState(false);
  const phase=activity.game==="snack"||activity.phase==="blocked"?activity.phase:"off";
  const act=async(action:string)=>{setBusy(true);setError("");try{await invoke("snack_action",{action,difficulty});}catch(e){setError(String(e));}finally{setBusy(false);}};
  return <section className="play-card" data-testid="search-panel">
    <h2>藏零食找一找</h2><p role="status" data-testid="snack-phase" data-phase={phase}>{PHASES[phase]||phase}</p>
    <label className="small">藏在哪里 <select aria-label="寻找难度" value={difficulty} onChange={e=>setDifficulty(e.target.value)}><option value="easy">明显一点</option><option value="far">稍远一点</option></select></label>
    <div className="play-actions"><button disabled={busy} onClick={()=>void act("place")}>藏一块零食</button><button className="primary" disabled={busy||!['placing','ready'].includes(phase)} onClick={()=>void act("find")}>开始寻找</button></div>
    <button className="quiet" onClick={()=>void act("cancel")}>结束找零食</button>
    <p className="small">本次找到 <span data-testid="found-count">{activity.finds}</span> 次。也可以拖到别的位置再找。</p>
    {error||loadError?<ActionError error={error||loadError} message="这次寻找没能完成，请重新放置零食再试；专注期间先结束专注。"/>:null}
  </section>;
}
export function HiddenTreat(){return <div className="toy-stage"><svg width="28" height="28" viewBox="0 0 28 28" role="img" aria-label="拖动这块零食" data-testid="hidden-cookie"><circle cx="14" cy="14" r="12" fill="#dfb46e" stroke="#926037" strokeWidth="2"/><g fill="#80502f"><circle cx="9" cy="8" r="2"/><circle cx="19" cy="10" r="2"/><circle cx="12" cy="18" r="2"/><circle cx="20" cy="19" r="1.6"/><circle cx="7" cy="17" r="1.4"/></g></svg></div>;}
