import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import ActionError from "./ActionError";

type Preferences={quiet_companion:boolean;playful_fetch:boolean;error:string|null};
export default function CompanionPreferences() {
  const [preferences,setPreferences]=useState<Preferences|null>(null);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  useEffect(()=>{
    let active=true;let received=false;
    const off=listen<Preferences>("pet:memory",e=>{received=true;if(active)setPreferences(e.payload);});
    off.then(()=>invoke<Preferences>("companion_status")).then(v=>{if(active&&!received)setPreferences(v);}).catch(e=>{if(active)setError(String(e));});
    return()=>{active=false;void off.then(f=>f()).catch(console.error);};
  },[]);
  const change=async(command:string,enabled:boolean)=>{
    setBusy(true);setError("");
    try{setPreferences(await invoke<Preferences>(command,{enabled}));}catch(e){setError(String(e));}finally{setBusy(false);}
  };
  return <section className="play-card" data-testid="companion-preferences">
    <h2>陪伴方式</h2>
    <label className="shortcut-switch"><input type="checkbox" data-testid="quiet-companion" disabled={busy||!preferences||!!preferences.error} checked={preferences?.quiet_companion||false} onChange={e=>void change("set_quiet_companion",e.target.checked)}/>安静陪伴</label>
    <p className="small" role="status">{preferences?.quiet_companion?"已开启：暂停主动走动、闲聊、喝水提醒和偶遇；你找大熊玩时照常回应。":"普通陪伴：保留主动走动、闲聊与偶遇。随时可以切换。"}</p>
    <label className="small playful-switch"><input type="checkbox" data-testid="playful-fetch" disabled={busy||!preferences||!!preferences.error} checked={preferences?.playful_fetch??true} onChange={e=>void change("set_playful_fetch",e.target.checked)}/>偶尔叼球逗你追</label>
    <p className="small">记在这台电脑上。安静陪伴不会改变原有的偶遇开关，也不影响已设置的专注计时提醒。</p>
    {error?<ActionError error={error} message="陪伴设置保存失败，原设置仍保留。请稍后再试。"/>:null}
  </section>;
}
