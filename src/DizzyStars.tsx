export type DizzyFeedback = {phase: "wobble" | "fall" | "down" | "recover"; x: number; y: number};

/** Pose and halo position share the native snapshot; orbiting needs no IPC ticks. */
export default function DizzyStars({dizzy}:{dizzy:DizzyFeedback|null|undefined}) {
  if (!dizzy) return null;
  return <div className="dizzy-stars" data-testid="dizzy-stars" data-phase={dizzy.phase}
    style={{left:dizzy.x,top:dizzy.y}} aria-hidden="true">
    <svg className="dizzy-ring" width="72" height="38" viewBox="0 0 72 38">
      <ellipse cx="36" cy="19" rx="26" ry="9" fill="none" stroke="#d79a43" strokeWidth="1" opacity=".4"/>
    </svg>
    {[0,1,2].map(i=>{
      const a=i*Math.PI*2/3;
      return <span className="dizzy-star" key={i} style={{animationDelay:`${-i/3}s`,transform:`translate(${Math.cos(a)*26}px,${Math.sin(a)*9}px)`}}>
        <svg width="16" height="16" viewBox="-8 -8 16 16"><path d="M0 -7 L2 -2 L7 -2 L3 2 L4 7 L0 4 L-4 7 L-3 2 L-7 -2 L-2 -2 Z"
          fill="#ffd36d" stroke="#9c6328" strokeWidth="1"/></svg>
      </span>;
    })}
  </div>;
}
