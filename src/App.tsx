import { useCallback, useEffect, useRef, useState } from "react";
import ToyDisc from "./ToyDisc";
import SpeechBubble from "./SpeechBubble";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Butterfly, type EncounterView } from "./EncountersPanel";
import { decodePetImage, BUILTIN_SRC, DEFAULT_ROWS, type PetSwitch } from "./pet-image";
import { DEFAULT_SPEECH, DAXIONG_SPEECH, mergeSpeech, pickSpeech, type SpeechTable, type SpeechHistory } from "./speech";

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

type Frame = { row: number; col: number; released_toy?: {toy: "ball" | "frisbee"; x: number; y: number} | null };

/**
 * 渲染层：不做行为决策，只把主进程推来的帧画出来；
 * 说话的内容从话术表随机挑（触发时机由主进程决定）。
 */
export default function App() {
  const [frame, setFrame] = useState<Frame>({ row: 0, col: 0 });
  const [bubble, setBubble] = useState<{ text: string; id: number; duration: number; protect: boolean } | null>(null);
  const [src, setSrc] = useState<string>(BUILTIN_SRC);
  const [rows, setRows] = useState<number>(DEFAULT_ROWS);
  const [clickable,setClickable]=useState(false);
  const [encounter,setEncounter]=useState<EncounterView>({kind:null,phase:"quiet",right:true});
  const [treating,setTreating] = useState(false);
  const [sleeping, setSleeping] = useState(false);

  // 图集的实际显示高度，随宠物切换
  const sheetH = rows * CELL_H;

  const speech = useRef<SpeechTable>(DEFAULT_SPEECH);
  const bubbleId = useRef(0);
  const speechHistory = useRef<SpeechHistory>(new Map());
  const lastText = useRef<string | null>(null);
  const readySent = useRef(false);
  const errorUntil = useRef(0);

  const dismissBubble = useCallback(() => setBubble(null), []);
  const protectBubble = useCallback((duration: number) => {
    if (bubble?.protect) errorUntil.current = Math.max(errorUntil.current, Date.now() + duration);
  }, [bubble?.id, bubble?.protect]);

  useEffect(() => {
    let alive = true;
    let revision = 0;
    let loading = false;
    let latestFrame: Frame = { row: 0, col: 0 };
    let activeRows = DEFAULT_ROWS;
    let treatTimer: number | null = null;
    const displayMessage = (text: string, duration = BUBBLE_MS, protect = duration > BUBBLE_MS) => {
      if (!alive) return;
      if (protect) errorUntil.current = Date.now() + duration;
      bubbleId.current += 1;
      setBubble({ text, id: bubbleId.current, duration, protect });
    };

    const applyPet = async (pet: PetSwitch) => {
      const request = ++revision;
      loading = true;
      try {
        const decoded = await decodePetImage(pet);
        if (!alive || request !== revision) return;
        speech.current = mergeSpeech(decoded.speech, pet.id === "__builtin__" ? DAXIONG_SPEECH : DEFAULT_SPEECH);
        speechHistory.current.clear();
        lastText.current = null;
        activeRows = decoded.rows;
        setRows(decoded.rows);
        setSrc(decoded.src);
        setFrame(latestFrame.row < activeRows ? latestFrame : { row: 0, col: 0 });
        loading = false;
        if (!readySent.current) {
          await invoke("pet_ready");
          if (alive) {
            readySent.current = true;
            try {if(!localStorage.getItem("pet-menu-intro-v2")){displayMessage("右键我就能一起玩。Mac 可双指点按；没看清时，托盘里有玩法说明。",8000,false);}}catch { /* The menu still works when browser storage is unavailable. */ }
          }
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
      if (!alive || Date.now() < errorUntil.current) return;
      const text = pickSpeech(speech.current, event.payload, lastText.current, speechHistory.current);
      if (!text) return;
      lastText.current = text;
      displayMessage(text);
    });

    const offSwitch = listen<PetSwitch>("pet:switch", (event) => {
      if (alive) void applyPet(event.payload);
    });

    // 睡眠之类的状态由主进程决定，前端只负责视觉表现
    const offState = listen<{ sleeping: boolean; clickable:boolean }>("pet:state", (event) => {
      if (alive) { setSleeping(event.payload.sleeping);setClickable(event.payload.clickable); }
    });

    const offEncounter=listen<EncounterView>("pet:encounter",e=>{if(alive) setEncounter(e.payload);});
    const offIntro=listen("pet:onboarding-complete",()=>{try{localStorage.setItem("pet-menu-intro-v2","seen");}catch{/* Opening menus still works without storage. */}});
    const offMessage = listen<string>("pet:message",event => {
      if(alive && Date.now()>=errorUntil.current) displayMessage(event.payload);
    });
    const offTreat = listen("pet:treat",()=>{
      if(!alive) return;
      setTreating(true);
      if(treatTimer!==null) window.clearTimeout(treatTimer);
      treatTimer=window.setTimeout(()=>{if(alive) setTreating(false);},1800);
    });
    const offError = listen<string>("pet:error", (event) => {
      errorUntil.current = Date.now() + 7000;
      displayMessage(event.payload, 7000);
    });

    // 监听注册完成再取快照，避免启动时漏掉帧 / 切换事件。
    Promise.all([offFrame, offSay, offSwitch, offState, offError, offMessage, offTreat, offEncounter, offIntro]).then(async () => {
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
      if(treatTimer!==null) window.clearTimeout(treatTimer);
      offFrame.then((off) => off()).catch(console.error);
      offSay.then((off) => off()).catch(console.error);
      offSwitch.then((off) => off()).catch(console.error);
      offState.then((off) => off()).catch(console.error);
      offError.then((off) => off()).catch(console.error);
      offMessage.then(off=>off()).catch(console.error);
      offTreat.then(off=>off()).catch(console.error);
      offEncounter.then(off=>off()).catch(console.error);
      offIntro.then(off=>off()).catch(console.error);
    };
  }, []);

  return (
    <div className="stage" onContextMenu={e => { e.preventDefault(); void invoke("open_pet_menu").catch(error=>{setBubble({id:++bubbleId.current,text:`菜单没打开：${String(error)}`,duration:5000,protect:true});}); }}>
      {bubble && (
        <SpeechBubble key={bubble.id} text={bubble.text} duration={bubble.duration} onDismiss={dismissBubble} onDuration={protectBubble}/>
      )}
      <Butterfly event={encounter}/>
      {treating && (src!==BUILTIN_SRC || rows<21) ? <div className="treat-cookie" data-testid="treat-cookie" aria-hidden="true"><i/><i/><i/></div> : null}
      {frame.released_toy ? <div className="released-toy" data-testid="released-toy" data-toy={frame.released_toy.toy}
        style={{left:frame.released_toy.x,top:frame.released_toy.y}} aria-hidden="true">
        {frame.released_toy.toy==="frisbee" ? <ToyDisc/> : <div className="toy-ball"/>}
      </div> : null}
      {/* 用一个裁剪窗口套住整张图集，靠 transform 平移来切帧 */}
      <div className={`pet-clip${sleeping ? " sleeping" : ""}`} data-testid="pet" data-clickable={clickable} data-sleeping={sleeping} data-row={frame.row} data-col={frame.col}>
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
