import {createContext,useContext,useEffect,useMemo,useState,type ReactNode} from "react";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
export type Activity={id:number;kind:string|null;phase:string;rewardable:boolean;game:string;treat:[number,number]|null;difficulty:string;finds:number};
const INITIAL:Activity={id:0,kind:null,phase:"off",rewardable:false,game:"none",treat:null,difficulty:"easy",finds:0};
const Context=createContext<{activity:Activity;error:string}|null>(null);
export function ActivityProvider({children}:{children:ReactNode}) {
  const [activity,setActivity]=useState(INITIAL);
  const [error,setError]=useState("");
  useEffect(()=>{
    let active=true;let received=false;
    const off=listen<Activity>("pet:activity",e=>{received=true;if(active)setActivity(e.payload);});
    off.then(()=>invoke<Activity>("trick_status")).then(v=>{if(active&&!received)setActivity(v);}).catch(e=>{if(active)setError(String(e));});
    return ()=>{active=false;void off.then(f=>f()).catch(console.error);};
  },[]);
  const value=useMemo(()=>({activity,error}),[activity,error]);
  return <Context.Provider value={value}>{children}</Context.Provider>;
}
export function useActivity(){const v=useContext(Context);if(!v)throw new Error("Missing ActivityProvider");return v;}
