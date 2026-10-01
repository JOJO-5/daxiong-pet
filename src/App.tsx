import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { decodePetImage, BUILTIN_SRC, DEFAULT_ROWS, type PetSwitch } from "./pet-image";
import { DEFAULT_SPEECH, mergeSpeech, pickSpeech, type SpeechTable } from "./speech";

// 精灵图契约：固定 8 列，单元格 192x208
const SHEET_W = 1536;
const CELL_W = 192;
const CELL_H = 208;

// 行数由后端下发（11 = 完整含注视；9 = 无注视的降级图集）。
// 不能写死高度：8x9 的图集只有 1872 高，按 2288 渲染会把图片纵向拉伸导致切帧错位。


// 192*0.75=144、208*0.75=156，与 engine.rs 的 PET_W / PET_H 对应
const SCALE = 0.75;

/** 气泡停留时长（毫秒） */
const BUBBLE_MS = 2600;

type Frame = { row: number; col: number };

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
    let revision = 0;
    let loading = false;
    let latestFrame: Frame = { row: 0, col: 0 };
    let activeRows = DEFAULT_ROWS;
    let errorUntil = 0;
    const displayMessage = (text: string, duration = BUBBLE_MS) => {
      if (!alive) return;
      bubbleId.current += 1;
      setBubble({ text, id: bubbleId.current });
      if (hideTimer.current !== null) window.clearTimeout(hideTimer.current);
      hideTimer.current = window.setTimeout(() => { if (alive) setBubble(null); }, duration);
    };

    const applyPet = async (pet: PetSwitch) => {
      const request = ++revision;
      loading = true;
      try {
        const decoded = await decodePetImage(pet);
        if (!alive || request !== revision) return;
        speech.current = mergeSpeech(decoded.speech);
        activeRows = decoded.rows;
        setRows(decoded.rows);
        setSrc(decoded.src);
        setFrame(latestFrame.row < activeRows ? latestFrame : { row: 0, col: 0 });
        loading = false;
        if (!readySent.current) {
          await invoke("pet_ready");
          if (alive) readySent.current = true;
        }
      } catch (error) {
        if (!alive || request !== revision) return;
        loading = false;
        displayMessage(`宠物加载失败：${String(error)}。已尝试恢复大熊。`, 7000);
        if (pet.id !== "__builtin__") {
          invoke("set_pet", { id: "__builtin__" }).catch((err) => displayMessage(`恢复大熊失败：${String(err)}`, 7000));
        }
      }
    };

    const offFrame = listen<Frame>("pet:frame", (event) => {
      latestFrame = event.payload;
      if (alive && !loading && event.payload.row < activeRows) setFrame(event.payload);
    });

    const offSay = listen<string>("pet:say", (event) => {
      if (!alive || Date.now() < errorUntil) return;
      const text = pickSpeech(speech.current, event.payload, lastText.current);
      if (!text) return;
      lastText.current = text;
      displayMessage(text);
    });

    const offSwitch = listen<PetSwitch>("pet:switch", (event) => {
      if (alive) void applyPet(event.payload);
    });

    // 睡眠之类的状态由主进程决定，前端只负责视觉表现
    const offState = listen<{ sleeping: boolean }>("pet:state", (event) => {
      if (alive) setSleeping(event.payload.sleeping);
    });

    const offError = listen<string>("pet:error", (event) => {
      errorUntil = Date.now() + 7000;
      displayMessage(event.payload, 7000);
    });

    // 监听注册完成再取快照，避免启动时漏掉帧 / 切换事件。
    Promise.all([offFrame, offSay, offSwitch, offState, offError]).then(async () => {
      const observed = revision;
      const pet = await invoke<PetSwitch>("current_pet");
      if (alive && revision === observed) await applyPet(pet);
    }).catch((error) => {
      if (!alive) return;
      displayMessage(`读取宠物失败：${String(error)}。正在恢复大熊。`, 7000);
      invoke("set_pet", { id: "__builtin__" }).catch((err) => displayMessage(String(err), 7000));
    });

    return () => {
      alive = false;
      if (hideTimer.current !== null) window.clearTimeout(hideTimer.current);
      offFrame.then((off) => off()).catch(console.error);
      offSay.then((off) => off()).catch(console.error);
      offSwitch.then((off) => off()).catch(console.error);
      offState.then((off) => off()).catch(console.error);
      offError.then((off) => off()).catch(console.error);
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
