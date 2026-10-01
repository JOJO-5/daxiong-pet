import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { DEFAULT_SPEECH, mergeSpeech, pickSpeech, type SpeechTable } from "./speech";

// 精灵图契约：固定 8 列，单元格 192x208
const SHEET_W = 1536;
const CELL_W = 192;
const CELL_H = 208;

// 行数由后端下发（11 = 完整含注视；9 = 无注视的降级图集）。
// 不能写死高度：8x9 的图集只有 1872 高，按 2288 渲染会把图片纵向拉伸导致切帧错位。
const DEFAULT_ROWS = 11;

// 192*0.75=144、208*0.75=156，与 engine.rs 的 PET_W / PET_H 对应
const SCALE = 0.75;

/** 气泡停留时长（毫秒） */
const BUBBLE_MS = 2600;

/** 内置宠物（随程序打包）；外部宠物包读取失败时也回落到它 */
const BUILTIN_SRC = "/spritesheet.webp";

type Frame = { row: number; col: number };

type PetSwitch = {
  id: string;
  name: string;
  rows: number;
  data_url: string | null;
  /** 宠物包自带的 speech.json（可选） */
  speech?: unknown;
};

/**
 * 渲染层：不做行为决策，只把主进程推来的帧画出来；
 * 说话的内容从话术表随机挑（触发时机由主进程决定）。
 */
export default function App() {
  const [frame, setFrame] = useState<Frame>({ row: 0, col: 0 });
  const [bubble, setBubble] = useState<{ text: string; id: number } | null>(null);
  const [src, setSrc] = useState<string>(BUILTIN_SRC);
  const [rows, setRows] = useState<number>(DEFAULT_ROWS);
  const [sleeping, setSleeping] = useState(false);

  // 图集的实际显示高度，随宠物切换
  const sheetH = rows * CELL_H;

  const speech = useRef<SpeechTable>(DEFAULT_SPEECH);
  const bubbleId = useRef(0);
  const lastText = useRef<string | null>(null);
  const hideTimer = useRef<number | null>(null);
  const readySent = useRef(false);

  useEffect(() => {
    let alive = true;

    /** 等图片解码完再通知主进程显示窗口，否则透明窗口会出现一帧白闪 */
    const markReady = (url: string) => {
      if (readySent.current) return;
      const img = new Image();
      img.src = url;
      img.decode()
        .catch(() => undefined)
        .then(() => {
          if (alive && !readySent.current) {
            readySent.current = true;
            invoke("pet_ready").catch(() => undefined);
          }
        });
    };

    /** 应用一只宠物：换图、重置帧、套用它的话术 */
    const applyPet = (pet: PetSwitch) => {
      speech.current = mergeSpeech(pet.speech);
      const url = pet.data_url ?? BUILTIN_SRC;
      setRows(pet.rows > 0 ? pet.rows : DEFAULT_ROWS);
      setSrc(url);
      setFrame({ row: 0, col: 0 });
      return url;
    };

    const offFrame = listen<Frame>("pet:frame", (event) => {
      if (alive) setFrame(event.payload);
    });

    const offSay = listen<string>("pet:say", (event) => {
      if (!alive) return;
      const text = pickSpeech(speech.current, event.payload, lastText.current);
      if (!text) return;
      lastText.current = text;
      bubbleId.current += 1;
      // key 变化会重新挂载气泡，从而重播入场动画（连点时也有反馈）
      setBubble({ text, id: bubbleId.current });

      if (hideTimer.current !== null) window.clearTimeout(hideTimer.current);
      hideTimer.current = window.setTimeout(() => {
        if (alive) setBubble(null);
      }, BUBBLE_MS);
    });

    const offSwitch = listen<PetSwitch>("pet:switch", (event) => {
      if (alive) applyPet(event.payload);
    });

    // 睡眠之类的状态由主进程决定，前端只负责视觉表现
    const offState = listen<{ sleeping: boolean }>("pet:state", (event) => {
      if (alive) setSleeping(event.payload.sleeping);
    });

    // 取当前该显示哪只宠物
    invoke<PetSwitch>("current_pet")
      .then((pet) => {
        if (alive) markReady(applyPet(pet));
      })
      .catch(() => {
        if (alive) markReady(BUILTIN_SRC);
      });

    return () => {
      alive = false;
      if (hideTimer.current !== null) window.clearTimeout(hideTimer.current);
      offFrame.then((off) => off());
      offSay.then((off) => off());
      offSwitch.then((off) => off());
      offState.then((off) => off());
    };
  }, []);

  return (
    <div className="stage">
      {bubble && (
        <div className="bubble" key={bubble.id}>
          {bubble.text}
        </div>
      )}
      {/* 用一个裁剪窗口套住整张图集，靠 transform 平移来切帧 */}
      <div className={`pet-clip${sleeping ? " sleeping" : ""}`}>
        <img
          className="pet-sheet"
          alt=""
          draggable={false}
          src={src}
          style={{
            width: SHEET_W * SCALE,
            height: sheetH * SCALE,
            transform: `translate(${-frame.col * CELL_W * SCALE}px, ${
              -frame.row * CELL_H * SCALE
            }px)`,
          }}
        />
      </div>
    </div>
  );
}
