import { useEffect, useState } from "react";
import { ropeGeometry, type TugView } from "./tug-view";

export default function TugRope({ rope }: { rope: TugView }) {
  const [size, setSize] = useState(() => ({ width: innerWidth, height: innerHeight, scale: devicePixelRatio }));
  useEffect(() => {
    const resize = () => setSize({ width: innerWidth, height: innerHeight, scale: devicePixelRatio });
    window.addEventListener("resize", resize);
    return () => window.removeEventListener("resize", resize);
  }, []);
  const g = ropeGeometry(rope, size.width, size.height, size.scale);
  const path = `M ${g.x1} ${g.y1} Q ${size.width / 2} ${size.height / 2 + g.sag} ${g.x2} ${g.y2}`;
  return <svg className="tug-rope" width="100%" height="100%" data-testid="tug-rope" role="img" aria-label="拔河绳，按住橙色绳环向外轻拉，松手休息">
    <title>按住橙色绳环，向外轻拉，松手休息</title>
    <path d={path} fill="none" stroke="#684827" strokeWidth="7" strokeLinecap="round" />
    <path d={path} fill="none" stroke="#e2bb78" strokeWidth="4" strokeLinecap="round" strokeDasharray="4 3" />
    <circle cx={g.x1} cy={g.y1} r="3" fill="#684827" />
    <circle cx={g.x2} cy={g.y2} r="12" fill="#fff4d8" fillOpacity=".8" stroke="#a25b2b" strokeWidth="6" />
    <circle cx={g.x2} cy={g.y2} r="12" fill="none" stroke="#f2a455" strokeWidth="3" />
  </svg>;
}
