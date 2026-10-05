import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import ActionError from "./ActionError";
type Shortcut={requested:boolean;enabled:boolean;key:string;error:string|null};
export default function PreferencesPanel(){
 const [status,setStatus]=useState<Shortcut|null>(null);
 const [error,setError]=useState("");const [busy,setBusy]=useState(false);
 useEffect(()=>{let active=true;invoke<Shortcut>("shortcut_status").then(v=>{if(active)setStatus(v);}).catch(e=>{if(active)setError(String(e));});return()=>{active=false;};},[]);
 const change=async(enabled:boolean)=>{setBusy(true);setError("");try{setStatus(await invoke<Shortcut>("set_shortcut",{enabled}));}catch(e){setError(String(e));try{setStatus(await invoke<Shortcut>("shortcut_status"));}catch(readError){setError(String(readError));}}finally{setBusy(false);}};
 return <section className="play-card" data-testid="preferences-panel"><h2>打开方式</h2><p className="small">右键大熊可直接喂食或扔玩具。托盘也能打开互动面板。</p>
 <label className="small shortcut-switch"><input type="checkbox" data-testid="shortcut-switch" checked={status?.requested||false} disabled={busy||!status} onChange={e=>void change(e.target.checked)}/>启用 {status?.key||"Ctrl+Alt+P"}</label>
 <p className="small" role="status" data-testid="shortcut-status">{status?.enabled?"快捷键已生效：显示大熊并打开互动面板":"快捷键未启用，仍可用右键和托盘打开"}</p>
 <p className="small">Mac 上 Alt 对应 Option；Esc 可收起面板。</p>
 {error||status?.error?<ActionError error={error||status?.error||""} message="快捷键未生效或保存失败，仍可用右键和托盘打开。请关闭占用此快捷键的程序后重试。"/>:null}</section>;
}
