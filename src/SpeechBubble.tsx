import { invoke } from "@tauri-apps/api/core";
import { useEffect, useLayoutEffect, useRef, useState } from "react";

const PAGE_MS = 2600;
const MAX_HEIGHT = 70; // 78px above the pet, minus 8px for the border/shadow.

type Props = { text: string; duration: number; onDismiss: () => void; onDuration: (duration: number) => void; pending?: boolean };

/** Measure with the native platform font; long messages continue on readable pages. */
export default function SpeechBubble({ text, duration, onDismiss, onDuration, pending = false }: Props) {
  const element = useRef<HTMLDivElement>(null);
  const [pages, setPages] = useState([text]);
  const [page, setPage] = useState(0);
  const [hovered,setHovered]=useState(false);
  const [focused,setFocused]=useState(false);
  const paused=hovered||focused;
  const remaining=useRef(0);
  const pageKey=useRef("");

  useLayoutEffect(() => {
    const bubble = element.current;
    if (!bubble) return;
    const probe = bubble.cloneNode(false) as HTMLDivElement;
    probe.style.cssText = "visibility:hidden;animation:none;width:276px;";
    bubble.parentElement!.appendChild(probe);
    const characters = Array.from(text);
    const measured: string[] = [];
    try {
      let start = 0;
      while (start < characters.length) {
        let low = 1;
        let high = characters.length - start;
        while (low < high) {
          const middle = Math.ceil((low + high) / 2);
          probe.textContent = characters.slice(start, start + middle).join("");
          if (probe.getBoundingClientRect().height <= MAX_HEIGHT) low = middle;
          else high = middle - 1;
        }
        // Prefer a word boundary without dropping spaces, newlines or punctuation.
        let count = low;
        if (start + count < characters.length) {
          for (let i = count - 1; i >= Math.floor(count * .7); i--) {
            if (/\s/u.test(characters[start + i])) { count = i + 1; break; }
          }
        }
        measured.push(characters.slice(start, start + count).join(""));
        start += count;
      }
    } finally {
      probe.remove();
    }
    onDuration(Math.max(duration, PAGE_MS * measured.length));
    setPages(measured.length ? measured : [text]);
    setPage(0);
  }, [text, duration, onDuration]);

  useEffect(() => {
    if (paused || pending) return;
    const key=`${text}:${page}:${pages.length}:${duration}`;
    if (pageKey.current!==key) {pageKey.current=key;remaining.current=Math.max(PAGE_MS,duration/pages.length);}
    const started=performance.now();
    const timer = window.setTimeout(() => {
      if (page + 1 < pages.length) setPage(page + 1);
      else onDismiss();
    }, remaining.current);
    return () => {window.clearTimeout(timer);remaining.current=Math.max(0,remaining.current-(performance.now()-started));};
  }, [duration, onDismiss, page, pages, paused, pending, text]);

  useEffect(()=>{if(!paused&&!pending)return;onDuration(5000);const timer=window.setInterval(()=>onDuration(5000),2000);return()=>window.clearInterval(timer);},[paused,pending,onDuration]);
  useLayoutEffect(()=>{const r=element.current?.getBoundingClientRect();if(r)void invoke("pet_bubble_rect",{rect:[r.left,r.top,r.right,r.bottom]}).catch(console.error);},[pages,page]);
  useEffect(()=>()=>{void invoke("pet_bubble_rect",{rect:null}).catch(console.error);},[]);

  return <div className="bubble" ref={element} role="status" aria-label={text}
    tabIndex={0} aria-description="悬停或键盘聚焦时暂停；Esc收起" onMouseEnter={()=>setHovered(true)} onMouseLeave={()=>setHovered(false)} onFocus={()=>setFocused(true)} onBlur={()=>setFocused(false)} onKeyDown={e=>{if(e.key==="Escape")onDismiss();}} data-paused={paused} data-pending={pending}
    data-page={page + 1} data-pages={pages.length}>{pages[page]}</div>;
}
