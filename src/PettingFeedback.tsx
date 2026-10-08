export type TouchFeedback = {
  kind: "head" | "belly";
  phase: "waiting" | "head" | "down" | "belly" | "up";
  progress: number;
  x: number;
  y: number;
  stroking: boolean;
};

/** Decoration only: pointer decisions and animation frames share the native snapshot. */
export default function PettingFeedback({ touch }: { touch: TouchFeedback | null | undefined }) {
  if (!touch) return null;
  const active = touch.phase !== "waiting";
  return <div className={`petting-feedback${active ? " active" : ""}${touch.stroking ? " stroking" : ""}`}
    style={{ left: touch.x, top: touch.y }} data-testid="petting-feedback"
    data-kind={touch.kind} data-phase={touch.phase} aria-hidden="true">
    <svg className="touch-ring" width="22" height="22" viewBox="0 0 22 22">
      <circle className="touch-track" cx="11" cy="11" r="8"/>
      <circle className="touch-progress" cx="11" cy="11" r="8" pathLength="100"
        strokeDasharray="100" strokeDashoffset={100 - touch.progress}/>
    </svg>
    {active ? <><span className="touch-heart">♥</span><span className="touch-heart second">♥</span></> : null}
  </div>;
}
