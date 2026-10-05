import {useState} from "react";

export default function ActionError({error,message}:{error:string;message:string}) {
  const [expanded,setExpanded]=useState(false);
  return <div className="panel-error" role="alert">
    <p>{message}</p>
    <button className="quiet" aria-expanded={expanded} onClick={()=>setExpanded(v=>!v)}>{expanded?"收起详情":"查看详情"}</button>
    {expanded?<p className="small">{error}</p>:null}
  </div>;
}
