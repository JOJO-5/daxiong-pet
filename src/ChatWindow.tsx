import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./chat.css";

type Personality = { pet_name: string; owner_name: string; description: string; tone: string; length: string };
type Settings = { enabled: boolean; threads: number };
type Message = { id: number; role: string; content: string; created_at: number; status: string; request_id: string };
type Snapshot = { messages: Message[]; personality: Personality; defaults: Personality; settings: Settings; assets: { available: boolean; model: string; note: string }; loaded: boolean; active_request_id: string | null; active_preview: boolean };
type Reply = { request_id: string; content: string; status: string; preview: boolean; error: string | null };

export default function ChatWindow() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [draft, setDraft] = useState<Personality | null>(null);
  const [mode, setMode] = useState<"chat" | "personality">("chat");
  const [text, setText] = useState("");
  const [preview, setPreview] = useState("");
  const [busy, setBusy] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [confirmClear, setConfirmClear] = useState(false);
  const mounted = useRef(false);
  const initialized = useRef(false);
  const sending = useRef(false);
  const fetchRevision = useRef(0);
  const stream = useRef<Reply | null>(null);
  const log = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  const input = useRef<HTMLTextAreaElement>(null);

  const refresh = useCallback(async () => {
    const revision = ++fetchRevision.current;
    const data = await invoke<Snapshot>("ai_snapshot");
    if (!mounted.current || revision !== fetchRevision.current) return;
    if (data.active_request_id && !sending.current && stream.current?.request_id !== data.active_request_id) {
      const message = data.messages.find(m => m.role === "assistant" && m.request_id === data.active_request_id);
      stream.current = { request_id: data.active_request_id, content: message?.content || "", status: "generating", preview: data.active_preview, error: null };
    }
    setSnapshot(data);
    setMessages(data.messages.map(message => {
      const latest = stream.current;
      return latest && !latest.preview && latest.request_id === message.request_id && message.role === "assistant"
        ? { ...message, content: latest.content, status: latest.status === "generating" ? message.status : latest.status }
        : message;
    }));
    setBusy(!!data.active_request_id || sending.current);
    if (!initialized.current) { initialized.current = true; setDraft(data.personality); }
  }, []);

  useEffect(() => {
    let alive = true;
    mounted.current = true;
    const off = listen<Reply>("ai:reply", event => {
      const reply = event.payload;
      if (!alive || stream.current?.request_id !== reply.request_id) return;
      stream.current = reply;
      if (reply.preview) setPreview(reply.content);
      else setMessages(previous => previous.map(message => message.role === "assistant" && message.request_id === reply.request_id
        ? { ...message, content: reply.content, status: reply.status } : message));
      if (reply.status !== "generating") {
        sending.current = false;
        setBusy(false); setStopping(false);
        if (reply.error) setError(reply.error);
        void refresh().catch(e => { if (alive) setError(String(e)); });
      }
    });
    off.then(async () => {
      if (!alive) return;
      try { await refresh(); } catch (e) { if (alive) setError(String(e)); }
      finally { if (alive) void invoke("ai_ready").catch(e => setError(String(e))); }
    }).catch(e => { if (alive) { setError(String(e)); void invoke("ai_ready").catch(console.error); } });
    const focus = () => { if (alive) void refresh().catch(e => setError(String(e))); };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented && !(event.target instanceof HTMLSelectElement)) {
        event.preventDefault(); void invoke("close_chat").catch(e => setError(String(e)));
      }
    };
    const timer = window.setInterval(focus, 60_000);
    window.addEventListener("focus", focus); window.addEventListener("keydown", key);
    return () => { alive = false; mounted.current = false; window.clearInterval(timer); window.removeEventListener("focus", focus); window.removeEventListener("keydown", key); void off.then(fn => fn()).catch(console.error); };
  }, [refresh]);

  useEffect(() => { if (stick.current && log.current) log.current.scrollTop = log.current.scrollHeight; }, [messages]);

  const configure = async (settings: Settings) => {
    if (saving) return;
    setSaving(true); setError("");
    try { await invoke("ai_configure", { settings }); await refresh(); }
    catch (e) { setError(String(e)); }
    finally { setSaving(false); }
  };
  const send = async (isPreview = false) => {
    if (busy || sending.current || !snapshot?.settings.enabled || !snapshot.assets.available) return;
    const message = isPreview ? "今天有点累，不想听大道理，就想有只狗陪着。" : text.trim();
    if (!message || (isPreview && !draft)) return;
    const bytes = new Uint8Array(16);
    crypto.getRandomValues(bytes);
    const id = Array.from(bytes, byte => byte.toString(16).padStart(2, "0")).join("");
    stream.current = { request_id: id, content: "", status: "generating", preview: isPreview, error: null };
    sending.current = true; setBusy(true); setStopping(false); setError(""); setNotice("");
    if (isPreview) setPreview("");
    else {
      stick.current = true;
      const base = { created_at: Math.floor(Date.now() / 1000), request_id: id };
      setMessages(previous => [...previous, { ...base, id: -1, role: "user", content: message, status: "complete" }, { ...base, id: -2, role: "assistant", content: "", status: "generating" }]);
      setText("");
    }
    try {
      await invoke("ai_send", { requestId: id, text: message, preview: isPreview ? draft : null });
      sending.current = false;
      await refresh();
    } catch (e) {
      sending.current = false; setBusy(false); setError(String(e)); stream.current = null;
      if (!isPreview) setText(message);
      await refresh().catch(() => {});
    }
  };
  const stop = async () => {
    setStopping(true);
    try { await invoke("ai_cancel", { requestId: stream.current?.request_id || snapshot?.active_request_id || "" }); }
    catch (e) { setError(String(e)); setStopping(false); }
  };
  const savePersonality = async () => {
    if (!draft || saving) return;
    setSaving(true); setError(""); setNotice("");
    try {
      const saved = await invoke<Personality>("ai_save_personality", { personality: draft });
      setDraft(saved); await refresh(); setNotice("性格已保存，下一条回复就会使用。");
    } catch (e) { setError(String(e)); }
    finally { setSaving(false); }
  };
  const clearChat = async () => {
    setSaving(true); setError("");
    try { await invoke("ai_clear_chat"); stream.current = null; await refresh(); setConfirmClear(false); setNotice("聊天已清空，性格设置仍保留。"); }
    catch (e) { setError(String(e)); }
    finally { setSaving(false); }
  };
  const pet = snapshot?.personality.pet_name || "大熊";
  const canSend = !!snapshot?.settings.enabled && snapshot.assets.available && !busy && !saving;
  return <main className="chat-window">
    <header className="chat-heading"><div><span className="chat-eyebrow">本地陪伴</span><h1>和{pet}聊聊</h1></div><button className="chat-icon-button" aria-label="收起聊天" title="收起聊天并停止回复（Esc）" onClick={() => void invoke("close_chat").catch(e => setError(String(e)))}>×</button></header>
    <nav className="chat-tabs" aria-label="聊天和性格"><button aria-pressed={mode === "chat"} onClick={() => setMode("chat")}>聊天</button><button aria-pressed={mode === "personality"} onClick={() => setMode("personality")}>性格与设置</button></nav>
    {error ? <div className="chat-error" role="alert">{error}<button aria-label="关闭错误提示" onClick={() => setError("")}>×</button></div> : null}
    {notice ? <p className="chat-notice" role="status">{notice}</p> : null}
    {!snapshot ? <div className="chat-loading"><p>{error ? "暂时无法读取聊天记录。" : "正在准备聊天…"}</p>{error ? <button onClick={() => void refresh().catch(e => setError(String(e)))}>重新读取</button> : null}</div> : <>
      {!snapshot.assets.available ? <aside className="chat-setup" role="status"><strong>本地模型还没准备好</strong><p>{snapshot.assets.note}</p><p>可以先编辑性格；准备好资源后重新打开聊天。</p></aside> : !snapshot.settings.enabled ? <aside className="chat-setup"><p>让{pet}陪你聊聊。首次回复需要加载本地模型，屏幕观察尚未开启。</p><button className="chat-primary" disabled={saving} onClick={() => void configure({ ...snapshot.settings, enabled: true })}>启用本地聊天</button></aside> : null}
      {mode === "chat" ? <>
        <div className="chat-log" ref={log} role="log" aria-label="聊天记录" aria-live={busy ? "off" : "polite"} onScroll={() => { if (log.current) stick.current = log.current.scrollHeight - log.current.scrollTop - log.current.clientHeight < 96; }}>
          {messages.length === 0 ? <div className="chat-empty"><span aria-hidden="true">🐾</span><h2>今天怎么样？</h2><p>开心的事、烦心的事，都可以慢慢聊。<br />对话保留在这台电脑上，默认最近30天。</p></div> : messages.map(message => <article key={`${message.request_id}-${message.role}`} className={`chat-message ${message.role}`} data-status={message.status}>
            <span className="chat-speaker">{message.role === "user" ? "你" : pet}</span><p>{message.content || (message.status === "generating" ? "正在准备回复…" : message.status === "cancelled" ? "这次回复已停止。" : "这次没有收到回复。")}</p>
            {message.status === "cancelled" ? <span className="chat-message-note">已停止</span> : message.status === "failed" ? <span className="chat-message-note">回复未完成 <button type="button" disabled={busy} onClick={() => { const original = messages.find(m => m.request_id === message.request_id && m.role === "user"); if (original) { setText(original.content); input.current?.focus(); } }}>重新编辑发送</button></span> : null}
          </article>)}
        </div>
        <form className="chat-compose" onSubmit={e => { e.preventDefault(); void send(); }}>
          <label className="chat-sr-only" htmlFor="chat-message">给{pet}的消息</label>
          <textarea ref={input} id="chat-message" value={text} maxLength={2000} placeholder="和我说说吧…" rows={2} disabled={!snapshot.settings.enabled || !snapshot.assets.available} onChange={e => setText(e.target.value)} onKeyDown={e => { if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); void send(); } }} />
          <div className="chat-compose-actions"><span className="chat-footnote" role="status">{busy ? stopping ? "正在停止…" : "正在回应，狗狗仍能陪你玩" : "Enter 发送 · Shift+Enter 换行"}</span>{busy ? <button type="button" disabled={stopping} onClick={() => void stop()}>停止回复</button> : <button className="chat-primary" type="submit" disabled={!canSend || !text.trim()}>发送</button>}</div>
        </form>
      </> : draft ? <div className="chat-settings">
        <section><h2>狗狗的性格</h2><p className="chat-footnote">调整说话方式，保存后影响下一次聊天。</p>
          <label htmlFor="ai-pet-name">狗狗叫什么</label><input id="ai-pet-name" maxLength={16} value={draft.pet_name} disabled={busy || saving} onChange={e => (setDraft({ ...draft, pet_name: e.target.value }), setPreview(""))} />
          <label htmlFor="ai-owner-name">聊天时怎么称呼你</label><input id="ai-owner-name" maxLength={16} placeholder="主人" value={draft.owner_name} disabled={busy || saving} onChange={e => (setDraft({ ...draft, owner_name: e.target.value }), setPreview(""))} />
          <label htmlFor="ai-description">性格描述</label><textarea id="ai-description" rows={3} maxLength={600} value={draft.description} disabled={busy || saving} onChange={e => (setDraft({ ...draft, description: e.target.value }), setPreview(""))} />
          <div className="chat-setting-grid"><label>语气<select aria-label="语气" value={draft.tone} disabled={busy || saving} onChange={e => (setDraft({ ...draft, tone: e.target.value }), setPreview(""))}><option value="gentle">温柔安静</option><option value="playful">活泼调皮</option></select></label><label>回复长短<select aria-label="回复长短" value={draft.length} disabled={busy || saving} onChange={e => (setDraft({ ...draft, length: e.target.value }), setPreview(""))}><option value="short">简短一两句</option><option value="normal">多聊几句</option></select></label></div>
          <div className="chat-setting-actions"><button className="chat-primary" disabled={busy || saving} onClick={() => void savePersonality()}>保存性格</button><button disabled={busy || saving} onClick={() => { setDraft(snapshot.personality); setPreview(""); setNotice("已放弃未保存的修改。"); }}>放弃修改</button><button disabled={busy || saving} onClick={() => { setDraft(snapshot.defaults); setPreview(""); setNotice("已恢复默认，保存后生效。"); }}>恢复默认</button></div>
          <div className="chat-preview"><h3>试聊这个性格</h3><p className="chat-footnote">试着回应“今天有点累”。使用当前编辑的性格，不加入聊天记录。</p><p className="chat-preview-reply">{preview || "点一下，听听狗狗怎么回应。"}</p>{busy ? <button disabled={stopping} onClick={() => void stop()}>停止回复</button> : <button disabled={!canSend} onClick={() => void send(true)}>试聊</button>}</div>
        </section>
        <section><h2>本地聊天</h2><p className="chat-footnote">{snapshot.assets.model} · {snapshot.loaded ? "已加载" : "尚未加载"}。空闲5分钟释放模型；这一版只聊文字。</p><label className="chat-switch"><input type="checkbox" checked={snapshot.settings.enabled} disabled={saving || !snapshot.assets.available} onChange={e => void configure({ ...snapshot.settings, enabled: e.target.checked })} />启用AI聊天</label><label>CPU占用<select aria-label="CPU占用" value={snapshot.settings.threads} disabled={busy || saving} onChange={e => void configure({ ...snapshot.settings, threads: Number(e.target.value) })}><option value={1}>最低（1线程）</option><option value={2}>轻占用（2线程）</option><option value={3}>适中（3线程）</option><option value={4}>更多CPU（4线程）</option></select></label></section>
        <section><h2>聊天记录</h2><p className="chat-footnote">默认保留最近30天。清空聊天会保留性格设置。</p>{confirmClear ? <div className="chat-setting-actions"><span>清空这台电脑上的聊天？</span><button disabled={busy || saving} onClick={() => void clearChat()}>确认清空</button><button onClick={() => setConfirmClear(false)}>保留聊天</button></div> : <button disabled={busy || saving} onClick={() => setConfirmClear(true)}>清空聊天</button>}</section>
      </div> : null}
    </>}
  </main>;
}
