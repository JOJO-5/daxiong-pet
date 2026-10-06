import { useEffect, useLayoutEffect, useRef, useState } from "react";

const PAGE_MS = 2600;
const MAX_HEIGHT = 70; // 78px above the pet, minus 8px for the border/shadow.

type Props = { text: string; duration: number; onDismiss: () => void; onDuration: (duration: number) => void };

/** Measure with the native platform font; long messages continue on readable pages. */
export default function SpeechBubble({ text, duration, onDismiss, onDuration }: Props) {
  const element = useRef<HTMLDivElement>(null);
  const [pages, setPages] = useState([text]);
  const [page, setPage] = useState(0);

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
    const timer = window.setTimeout(() => {
      if (page + 1 < pages.length) setPage(page + 1);
      else onDismiss();
    }, Math.max(PAGE_MS, duration / pages.length));
    return () => window.clearTimeout(timer);
  }, [duration, onDismiss, page, pages]);

  return <div className="bubble" ref={element} role="status" aria-label={text}
    data-page={page + 1} data-pages={pages.length}>{pages[page]}</div>;
}
