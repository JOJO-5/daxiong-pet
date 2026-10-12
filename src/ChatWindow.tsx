import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./chat.css";

type Personality = {
  pet_name: string;
  owner_name: string;
  description: string;
  tone: string;
  length: string;
};
type Settings = {
  enabled: boolean;
  acceleration_enabled: boolean;
  threads: number;
  backend: string;
  service_url: string;
  service_model: string;
  has_credential: boolean;
  remote_screen: boolean;
  local_model: string;
  local_context: number;
  idle_seconds: number;
  timeout_seconds: number;
  output_tokens: number;
  screen_backend: string;
  screen_service_url: string;
  screen_service_model: string;
  screen_has_credential: boolean;
  external_context: number;
  observe_enabled: boolean;
  speak_enabled: boolean;
  observe_seconds: number;
  speak_seconds: number;
  screen_scope: string;
  monitor_id: number | null;
  excluded_apps: string[];
  quiet_start: number;
  quiet_end: number;
  quiet_hours: boolean;
};
type Observation = {
  revision: number;
  status: string;
  busy: boolean;
  label: string | null;
  text: string | null;
  age_seconds: number | null;
  observations: number;
  speeches: number;
};
type Message = {
  id: number;
  role: string;
  content: string;
  created_at: number;
  status: string;
  request_id: string;
};
type Preference = {
  key: string;
  value: string;
  origin: string;
  revision: string;
};
type Snapshot = {
  observation: Observation;
  preferences: Preference[];
  messages: Message[];
  personality: Personality;
  defaults: Personality;
  settings: Settings;
  models: {
    id: string;
    name: string;
    bytes: number;
    installed: boolean;
    cached: boolean;
    purpose: string;
  }[];
  download: {
    id: string | null;
    received: number;
    total: number;
    status: string;
    busy: boolean;
  };
  acceleration: { available: boolean; installed: boolean; model_installed: boolean; component_bytes: number; model_bytes: number; busy: boolean; received: number; total: number; status: string };
  acceleration_runtime: { backend: string; note: string };
  acceleration_testing: boolean;
  assets: { available: boolean; model: string; note: string };
  loaded: boolean;
  active_request_id: string | null;
  active_preview: boolean;
};
type Reply = {
  request_id: string;
  content: string;
  status: string;
  preview: boolean;
  error: string | null;
};

export default function ChatWindow({ compact = false }: { compact?: boolean }) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [draft, setDraft] = useState<Personality | null>(null);
  const [mode, setMode] = useState<"chat" | "personality">("chat");
  const [text, setText] = useState("");
  const [preview, setPreview] = useState("");
  const [busy, setBusy] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [accelerationStarting, setAccelerationStarting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [confirmClear, setConfirmClear] = useState(false);
  const [serviceUrl, setServiceUrl] = useState("");
  const [serviceModel, setServiceModel] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [screenUrl, setScreenUrl] = useState("");
  const [screenModel, setScreenModel] = useState("");
  const [screenKey, setScreenKey] = useState("");
  const [confirmReset, setConfirmReset] = useState(false);
  const [displays, setDisplays] = useState<
    { id: number; name: string; primary: boolean }[]
  >([]);
  const [screenReply, setScreenReply] = useState("");
  const [memorySearch, setMemorySearch] = useState("");
  const memoryRevision = useRef<string | null>(null);
  const draftDirty = useRef(false);
  const screenRevision = useRef(0);
  const editDraft = (value: Personality) => {
    draftDirty.current = true;
    setDraft(value);
  };
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
    if (
      data.active_request_id &&
      !sending.current &&
      stream.current?.request_id !== data.active_request_id
    ) {
      const message = data.messages.find(
        (m) =>
          m.role === "assistant" && m.request_id === data.active_request_id,
      );
      stream.current = {
        request_id: data.active_request_id,
        content: message?.content || "",
        status: "generating",
        preview: data.active_preview,
        error: null,
      };
    }
    const revisionText = data.preferences.map((p) => p.revision).join(",");
    if (
      memoryRevision.current !== null &&
      revisionText !== memoryRevision.current &&
      data.preferences.some((p) => p.origin === "user_chat")
    )
      setNotice("已记住你明确表达的偏好，可以在设置里修改或撤销。");
    memoryRevision.current = revisionText;
    if (!draftDirty.current) setDraft(data.personality);
    const observationStale = data.observation.revision < screenRevision.current;
    screenRevision.current = Math.max(
      screenRevision.current,
      data.observation.revision,
    );
    setSnapshot((previous) =>
      observationStale && previous
        ? { ...data, observation: previous.observation }
        : data,
    );
    setMessages(
      data.messages.map((message) => {
        const latest = stream.current;
        return latest &&
          !latest.preview &&
          latest.request_id === message.request_id &&
          message.role === "assistant" && message.status === "generating"
          ? {
              ...message,
              content: latest.content,
              status:
                latest.status === "generating" ? message.status : latest.status,
            }
          : message;
      }),
    );
    setBusy(!!data.active_request_id || sending.current);
    if (!initialized.current) {
      initialized.current = true;
      setDraft(data.personality);
      setServiceUrl(data.settings.service_url);
      setServiceModel(data.settings.service_model);
      setScreenUrl(data.settings.screen_service_url);
      setScreenModel(data.settings.screen_service_model);
    }
  }, []);

  useEffect(() => {
    let alive = true;
    mounted.current = true;
    const off = listen<Reply>("ai:reply", (event) => {
      const reply = event.payload;
      if (!alive) return;
      if (stream.current?.request_id !== reply.request_id) {
        if (sending.current) {void refresh().catch(e => setError(String(e)));return;}
        stream.current = reply;
        setBusy(reply.status === "generating");
        void refresh().catch(e => {if(alive) setError(String(e));});
      }
      stream.current = reply;
      if (reply.preview) setPreview(reply.content);
      else
        setMessages((previous) =>
          previous.map((message) =>
            message.role === "assistant" &&
            message.request_id === reply.request_id
              ? { ...message, content: reply.content, status: reply.status }
              : message,
          ),
        );
      if (reply.status !== "generating") {
        sending.current = false;
        setBusy(false);
        setStopping(false);
        if (reply.error) setError(reply.error);
        void refresh().catch((e) => {
          if (alive) setError(String(e));
        });
      }
    });
    const changed = listen("ai:changed", () => {
      if (alive) void refresh().catch(e => { if (alive) setError(String(e)); });
    });
    const cleared = listen("ai:cleared", () => {
      if (!alive) return;
      stream.current = null;
      setPreview("");
    });
    const focused = listen("ai:focus", () => {
      if (alive) input.current?.focus();
    });
    Promise.all([off, changed, cleared, focused])
      .then(async () => {
        if (!alive) return;
        try {
          await refresh();
        } catch (e) {
          if (alive) setError(String(e));
        } finally {
          if (alive) void invoke("ai_ready").catch((e) => setError(String(e)));
        }
      })
      .catch((e) => {
        if (alive) {
          setError(String(e));
          void invoke("ai_ready").catch(console.error);
        }
      });
    const offObserve = listen<Observation>("ai:observation", (e) => {
      if (alive && e.payload.revision >= screenRevision.current) {
        screenRevision.current = e.payload.revision;
        setSnapshot((old) => (old ? { ...old, observation: e.payload } : old));
      }
    });
    const offScreenReply = listen<{ text: string; revision: number }>(
      "ai:screen-reply",
      (e) => {
        if (alive && e.payload.revision >= screenRevision.current)
          setScreenReply(e.payload.text);
      },
    );
    void invoke<typeof displays>("ai_displays")
      .then((value) => {
        if (alive) setDisplays(value);
      })
      .catch(() => {});
    const screenTimer = window.setInterval(() => {
      if (alive) {
        setScreenReply("");
        setSnapshot((old) =>
          old
            ? {
                ...old,
                observation: {
                  ...old.observation,
                  text: null,
                  label: null,
                  age_seconds: null,
                },
              }
            : old,
        );
      }
    }, 120_000);
    const focus = () => {
      if (alive) void refresh().catch((e) => setError(String(e)));
    };
    const key = (event: KeyboardEvent) => {
      if (
        event.key === "Escape" &&
        !event.defaultPrevented &&
        !(event.target instanceof HTMLSelectElement)
      ) {
        event.preventDefault();
        void invoke("close_chat").catch((e) => setError(String(e)));
      }
    };
    const timer = window.setInterval(focus, 60_000);
    window.addEventListener("focus", focus);
    window.addEventListener("keydown", key);
    return () => {
      alive = false;
      mounted.current = false;
      window.clearInterval(timer);
      window.clearInterval(screenTimer);
      void offObserve.then((fn) => fn()).catch(console.error);
      void offScreenReply.then((fn) => fn()).catch(console.error);
      window.removeEventListener("focus", focus);
      window.removeEventListener("keydown", key);
      void off.then((fn) => fn()).catch(console.error);
      void changed.then(fn=>fn()).catch(console.error);
      void cleared.then(fn=>fn()).catch(console.error);
      void focused.then(fn=>fn()).catch(console.error);
    };
  }, [refresh]);

  useEffect(() => {
    if (stick.current && log.current)
      log.current.scrollTop = log.current.scrollHeight;
  }, [messages]);

  const configure = async (settings: Settings) => {
    if (saving) return;
    setSaving(true);
    setError("");
    try {
      await invoke("ai_configure", { settings });
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };
  const send = async (isPreview = false) => {
    if (
      busy ||
      sending.current ||
      !snapshot?.settings.enabled ||
      (snapshot.settings.backend === "bundled" && !snapshot.assets.available)
    )
      return;
    const message = isPreview
      ? "今天有点累，不想听大道理，就想有只狗陪着。"
      : text.trim();
    if (!message || (isPreview && !draft)) return;
    const bytes = new Uint8Array(16);
    crypto.getRandomValues(bytes);
    const id = Array.from(bytes, (byte) =>
      byte.toString(16).padStart(2, "0"),
    ).join("");
    stream.current = {
      request_id: id,
      content: "",
      status: "generating",
      preview: isPreview,
      error: null,
    };
    sending.current = true;
    setBusy(true);
    setStopping(false);
    setError("");
    setNotice("");
    if (isPreview) setPreview("");
    else {
      stick.current = true;
      const base = {
        created_at: Math.floor(Date.now() / 1000),
        request_id: id,
      };
      setMessages((previous) => [
        ...previous,
        { ...base, id: -1, role: "user", content: message, status: "complete" },
        {
          ...base,
          id: -2,
          role: "assistant",
          content: "",
          status: "generating",
        },
      ]);
      setText("");
    }
    try {
      await invoke("ai_send", {
        requestId: id,
        text: message,
        preview: isPreview ? draft : null,
      });
      sending.current = false;
      await refresh();
    } catch (e) {
      sending.current = false;
      setBusy(false);
      setError(String(e));
      stream.current = null;
      if (!isPreview) setText(message);
      await refresh().catch(() => {});
    }
  };
  const stop = async () => {
    setStopping(true);
    try {
      await invoke("ai_cancel", {
        requestId:
          stream.current?.request_id || snapshot?.active_request_id || "",
      });
    } catch (e) {
      setError(String(e));
      setStopping(false);
    }
  };
  useEffect(() => {
    if (!snapshot?.download.busy && !snapshot?.acceleration.busy && !snapshot?.acceleration_testing && !accelerationStarting) return;
    const timer = window.setInterval(
      () => void refresh().catch((e) => setError(String(e))),
      accelerationStarting ? 250 : 2000,
    );
    return () => window.clearInterval(timer);
  }, [snapshot?.download.busy, snapshot?.acceleration.busy, snapshot?.acceleration_testing, accelerationStarting, refresh]);
  const savePersonality = async () => {
    if (!draft || saving) return;
    setSaving(true);
    setError("");
    setNotice("");
    try {
      const saved = await invoke<Personality>("ai_save_personality", {
        personality: draft,
      });
      draftDirty.current = false;
      setDraft(saved);
      await refresh();
      setNotice("性格已保存，下一条回复就会使用。");
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };
  const clearChat = async () => {
    setSaving(true);
    setError("");
    try {
      await invoke("ai_clear_chat");
      stream.current = null;
      await refresh();
      setConfirmClear(false);
      setNotice("聊天已清空，性格设置仍保留。");
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };
  const pet = snapshot?.personality.pet_name || "大熊";
  const canSend =
    !!snapshot?.settings.enabled &&
    (snapshot.settings.backend === "external" || snapshot.assets.available) &&
    !busy &&
    !saving;
  if (compact) return (
    <main className="quick-chat" data-testid="quick-chat">
      <header>
        <strong>给{pet}说句话</strong>
        <button type="button" onClick={() => void invoke("open_chat").catch(e => setError(String(e)))}>
          聊天记录
        </button>
        <button type="button" aria-label="关闭小输入框" onClick={() => void invoke("close_chat").catch(e => setError(String(e)))}>
          ×
        </button>
      </header>
      <form onSubmit={e => { e.preventDefault(); void send(); }}>
        <textarea
          autoFocus
          ref={input}
          id="chat-message"
          aria-label={`给${pet}的消息`}
          placeholder={`和${pet}说说话…`}
          maxLength={2000}
          value={text}
          onChange={e => setText(e.target.value)}
          onKeyDown={e => {
            if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
              e.preventDefault();
              void send();
            }
          }}
        />
        <footer>
          <span role={error ? "alert" : "status"} title={error}>
            {error || (busy ? `${pet}正在想…` : !snapshot ? "正在准备…" :
              snapshot.settings.backend === "bundled" && !snapshot.assets.available ?
                "模型未准备好，请打开聊天记录" : "Enter 发送 · Shift+Enter 换行")}
          </span>
          {busy ? (
            <button type="button" disabled={stopping} onClick={() => void stop()}>
              {stopping ? "正在停止" : "停止"}
            </button>
          ) : snapshot && !snapshot.settings.enabled ? (
            <button type="button" disabled={saving} onClick={() => void configure({ ...snapshot.settings, enabled: true })}>
              启用聊天
            </button>
          ) : (
            <button type="submit" disabled={!canSend || !text.trim()}>发送</button>
          )}
        </footer>
      </form>
    </main>
  );
  return (
    <main className="chat-window">
      <header className="chat-heading">
        <div>
          <span className="chat-eyebrow">本地陪伴</span>
          <h1>和{pet}聊聊</h1>
        </div>
        <button
          className="chat-icon-button"
          aria-label="收起聊天"
          title="收起聊天并停止回复（Esc）"
          onClick={() =>
            void invoke("close_chat").catch((e) => setError(String(e)))
          }
        >
          ×
        </button>
      </header>
      <nav className="chat-tabs" aria-label="聊天和性格">
        <button aria-pressed={mode === "chat"} onClick={() => setMode("chat")}>
          聊天
        </button>
        <button
          aria-pressed={mode === "personality"}
          onClick={() => setMode("personality")}
        >
          性格与设置
        </button>
      </nav>
      {error ? (
        <div className="chat-error" role="alert">
          {error}
          <button aria-label="关闭错误提示" onClick={() => setError("")}>
            ×
          </button>
        </div>
      ) : null}
      {notice ? (
        <p className="chat-notice" role="status">
          {notice}
        </p>
      ) : null}
      {!snapshot ? (
        <div className="chat-loading">
          <p>{error ? "暂时无法读取聊天记录。" : "正在准备聊天…"}</p>
          {error ? (
            <button
              onClick={() => void refresh().catch((e) => setError(String(e)))}
            >
              重新读取
            </button>
          ) : null}
        </div>
      ) : (
        <>
          {!snapshot.assets.available &&
          snapshot.settings.backend === "bundled" ? (
            <aside className="chat-setup" role="status">
              <strong>本地模型还没准备好</strong>
              <p>{snapshot.assets.note}</p>
              <p>可以先编辑性格；准备好资源后重新打开聊天。</p>
            </aside>
          ) : !snapshot.settings.enabled ? (
            <aside className="chat-setup">
              <p>
                让{pet}陪你聊聊。首次回复需要加载本地模型，屏幕观察尚未开启。
              </p>
              <button
                className="chat-primary"
                disabled={saving}
                onClick={() =>
                  void configure({ ...snapshot.settings, enabled: true })
                }
              >
                启用本地聊天
              </button>
            </aside>
          ) : null}
          {mode === "chat" ? (
            <>
              <div
                className="chat-log"
                ref={log}
                role="log"
                aria-label="聊天记录"
                aria-live={busy ? "off" : "polite"}
                onScroll={() => {
                  if (log.current)
                    stick.current =
                      log.current.scrollHeight -
                        log.current.scrollTop -
                        log.current.clientHeight <
                      96;
                }}
              >
                {messages.length === 0 ? (
                  <div className="chat-empty">
                    <span aria-hidden="true">🐾</span>
                    <h2>今天怎么样？</h2>
                    <p>
                      开心的事、烦心的事，都可以慢慢聊。
                      <br />
                      对话保留在这台电脑上，默认最近30天。
                    </p>
                  </div>
                ) : (
                  messages.map((message) => (
                    <article
                      key={`${message.request_id}-${message.role}`}
                      className={`chat-message ${message.role}`}
                      data-status={message.status}
                    >
                      <span className="chat-speaker">
                        {message.role === "user" ? "你" : pet}
                      </span>
                      <p>
                        {message.content ||
                          (message.status === "generating"
                            ? "正在准备回复…"
                            : message.status === "cancelled"
                              ? "这次回复已停止。"
                              : "这次没有收到回复。")}
                      </p>
                      {message.status === "cancelled" ? (
                        <span className="chat-message-note">已停止</span>
                      ) : message.status === "failed" ? (
                        <span className="chat-message-note">
                          回复未完成{" "}
                          <button
                            type="button"
                            disabled={busy}
                            onClick={() => {
                              const original = messages.find(
                                (m) =>
                                  m.request_id === message.request_id &&
                                  m.role === "user",
                              );
                              if (original) {
                                setText(original.content);
                                input.current?.focus();
                              }
                            }}
                          >
                            重新编辑发送
                          </button>
                        </span>
                      ) : null}
                    </article>
                  ))
                )}
              </div>
              <form
                className="chat-compose"
                onSubmit={(e) => {
                  e.preventDefault();
                  void send();
                }}
              >
                <label className="chat-sr-only" htmlFor="chat-message">
                  给{pet}的消息
                </label>
                <textarea
                  ref={input}
                  id="chat-message"
                  value={text}
                  maxLength={2000}
                  placeholder="和我说说吧…"
                  rows={2}
                  disabled={
                    !snapshot.settings.enabled ||
                    (snapshot.settings.backend === "bundled" &&
                      !snapshot.assets.available)
                  }
                  onChange={(e) => setText(e.target.value)}
                  onKeyDown={(e) => {
                    if (
                      e.key === "Enter" &&
                      !e.shiftKey &&
                      !e.nativeEvent.isComposing
                    ) {
                      e.preventDefault();
                      void send();
                    }
                  }}
                />
                <div className="chat-compose-actions">
                  <span className="chat-footnote" role="status">
                    {busy
                      ? stopping
                        ? "正在停止…"
                        : "正在回应，狗狗仍能陪你玩"
                      : "Enter 发送 · Shift+Enter 换行"}
                  </span>
                  {busy ? (
                    <button
                      type="button"
                      disabled={stopping}
                      onClick={() => void stop()}
                    >
                      停止回复
                    </button>
                  ) : (
                    <button
                      className="chat-primary"
                      type="submit"
                      disabled={!canSend || !text.trim()}
                    >
                      发送
                    </button>
                  )}
                </div>
              </form>
            </>
          ) : draft ? (
            <div className="chat-settings">
              <section>
                <h2>狗狗的性格</h2>
                <p className="chat-footnote">
                  调整说话方式，保存后影响下一次聊天。
                </p>
                <label htmlFor="ai-pet-name">狗狗叫什么</label>
                <input
                  id="ai-pet-name"
                  maxLength={16}
                  value={draft.pet_name}
                  disabled={busy || saving}
                  onChange={(e) => (
                    editDraft({ ...draft, pet_name: e.target.value }),
                    setPreview("")
                  )}
                />
                <label htmlFor="ai-owner-name">聊天时怎么称呼你</label>
                <input
                  id="ai-owner-name"
                  maxLength={16}
                  placeholder="主人"
                  value={draft.owner_name}
                  disabled={busy || saving}
                  onChange={(e) => (
                    editDraft({ ...draft, owner_name: e.target.value }),
                    setPreview("")
                  )}
                />
                <label htmlFor="ai-description">性格描述</label>
                <textarea
                  id="ai-description"
                  rows={3}
                  maxLength={600}
                  value={draft.description}
                  disabled={busy || saving}
                  onChange={(e) => (
                    editDraft({ ...draft, description: e.target.value }),
                    setPreview("")
                  )}
                />
                <div className="chat-setting-grid">
                  <label>
                    语气
                    <select
                      aria-label="语气"
                      value={draft.tone}
                      disabled={busy || saving}
                      onChange={(e) => (
                        editDraft({ ...draft, tone: e.target.value }),
                        setPreview("")
                      )}
                    >
                      <option value="gentle">温柔安静</option>
                      <option value="playful">活泼调皮</option>
                    </select>
                  </label>
                  <label>
                    回复长短
                    <select
                      aria-label="回复长短"
                      value={draft.length}
                      disabled={busy || saving}
                      onChange={(e) => (
                        editDraft({ ...draft, length: e.target.value }),
                        setPreview("")
                      )}
                    >
                      <option value="short">简短一两句</option>
                      <option value="normal">多聊几句</option>
                    </select>
                  </label>
                </div>
                <div className="chat-setting-actions">
                  <button
                    className="chat-primary"
                    disabled={busy || saving}
                    onClick={() => void savePersonality()}
                  >
                    保存性格
                  </button>
                  <button
                    disabled={busy || saving}
                    onClick={() => {
                      draftDirty.current = false;
                      setDraft(snapshot.personality);
                      setPreview("");
                      setNotice("已放弃未保存的修改。");
                    }}
                  >
                    放弃修改
                  </button>
                  <button
                    disabled={busy || saving}
                    onClick={() => {
                      draftDirty.current = true;
                      setDraft(snapshot.defaults);
                      setPreview("");
                      setNotice("已恢复默认，保存后生效。");
                    }}
                  >
                    恢复默认
                  </button>
                </div>
                <div className="chat-preview">
                  <h3>试聊这个性格</h3>
                  <p className="chat-footnote">
                    试着回应“今天有点累”。使用当前编辑的性格，不加入聊天记录。
                  </p>
                  <p className="chat-preview-reply">
                    {preview || "点一下，听听狗狗怎么回应。"}
                  </p>
                  {busy ? (
                    <button disabled={stopping} onClick={() => void stop()}>
                      停止回复
                    </button>
                  ) : (
                    <button disabled={!canSend} onClick={() => void send(true)}>
                      试聊
                    </button>
                  )}
                </div>
              </section>
              <section>
                <h2>本地聊天</h2>
                <p className="chat-footnote">
                  {snapshot.assets.model} ·{" "}
                  {snapshot.loaded ? "已加载" : "尚未加载"}
                  。空闲{snapshot.settings.idle_seconds}
                  秒后释放模型。图片理解按需启动。
                </p>
                <label className="chat-switch">
                  <input
                    type="checkbox"
                    checked={snapshot.settings.enabled}
                    disabled={
                      saving ||
                      (snapshot.settings.backend === "bundled" &&
                        !snapshot.assets.available)
                    }
                    onChange={(e) =>
                      void configure({
                        ...snapshot.settings,
                        enabled: e.target.checked,
                      })
                    }
                  />
                  启用AI聊天
                </label>
                <label>
                  CPU占用
                  <select
                    aria-label="CPU占用"
                    value={snapshot.settings.threads}
                    disabled={busy || saving}
                    onChange={(e) =>
                      void configure({
                        ...snapshot.settings,
                        threads: Number(e.target.value),
                      })
                    }
                  >
                    <option value={1}>最低（1线程）</option>
                    <option value={2}>轻占用（2线程）</option>
                    <option value={3}>适中（3线程）</option>
                    <option value={4}>更多CPU（4线程）</option>
                  </select>
                </label>
              </section>
              <section>
                <h2>性能设置</h2>
                <p className="chat-footnote">
                  增加上下文或输出会占用更多内存和时间。图片理解按需加载2B视觉组件。
                </p>
                {(
                  [
                    {
                      key: "local_context",
                      label: "本地上下文上限",
                      min: 2048,
                      max: 8192,
                      step: 1024,
                    },
                    {
                      key: "external_context",
                      label: "外接上下文预算",
                      min: 1024,
                      max: 32768,
                      step: 1024,
                    },
                    {
                      key: "output_tokens",
                      label: "回复token上限",
                      min: 32,
                      max: 512,
                      step: 32,
                    },
                    {
                      key: "timeout_seconds",
                      label: "推理超时（秒）",
                      min: 30,
                      max: 300,
                      step: 30,
                    },
                    {
                      key: "idle_seconds",
                      label: "空闲释放（秒）",
                      min: 30,
                      max: 1800,
                      step: 30,
                    },
                  ] as const
                ).map((field) => (
                  <label key={`${field.key}:${snapshot.settings[field.key]}`}>
                    {field.label}
                    <input
                      type="number"
                      min={field.min}
                      max={field.max}
                      step={field.step}
                      disabled={busy || saving}
                      defaultValue={snapshot.settings[field.key]}
                      onBlur={(e) => {
                        const value = Number(e.target.value);
                        if (value !== snapshot.settings[field.key])
                          void configure({
                            ...snapshot.settings,
                            [field.key]: value,
                          });
                      }}
                    />
                  </label>
                ))}
              </section>
              <section className="chat-download-section" aria-label="可选GPU加速" data-testid="acceleration-settings">
                <h2>可选 GPU 加速 · 实验性</h2>
                <p className="chat-footnote">默认使用 CPU。下载 ML Drift 加速组件后可试运行 GPU 模式；需要单独的 0.8B 文字模型，回答能力可能比默认 2B 弱。图片理解和 OCR 继续使用原有 CPU。</p>
                <p className="chat-download-size">组件约 {(snapshot.acceleration.component_bytes / 1024 / 1024).toFixed(1)} MiB · 兼容模型约 {(snapshot.acceleration.model_bytes / 1024 / 1024).toFixed(0)} MiB</p>
                <p className="chat-footnote">安装与下载临时文件建议预留 2.5 GiB；下载完成后不会自动启用。</p>
                <div className="chat-download-state">
                  <p data-testid="acceleration-backend">当前：{snapshot.settings.backend === "external" ? "外接服务" : snapshot.acceleration_runtime.backend === "ml-drift-gpu" ? "ML Drift GPU模式（实验性）" : "CPU"}</p>
                  <p className="chat-footnote" role="status">{snapshot.acceleration_runtime.note}</p>
                </div>
                {!snapshot.acceleration.available ? <p>此平台组件尚未发布，继续使用 CPU。</p> : null}
                <div className="chat-download-actions">
                  <button className="chat-primary chat-download-primary" disabled={busy || saving || snapshot.acceleration.busy || !snapshot.acceleration.available} onClick={() => {
                    void invoke("ai_acceleration_download").then(refresh).catch((e) => setError(String(e)));
                  }}>{snapshot.acceleration.installed && snapshot.acceleration.model_installed ? "校验/修复加速下载" : "下载加速组件与模型"}</button>
                  <button disabled={busy || saving || snapshot.acceleration.busy || !snapshot.acceleration.installed || !snapshot.acceleration.model_installed || snapshot.settings.backend !== "bundled"} onClick={async () => {
                    setSaving(true); setAccelerationStarting(true); setError("");
                    try { await invoke("ai_acceleration_enable"); } catch(e) { setError(String(e)); }
                    finally { await refresh().catch((e) => setError(String(e))); setSaving(false); setAccelerationStarting(false); }
                  }}>{snapshot.acceleration_testing ? "正在试运行…" : "试运行并启用加速"}</button>
                  <button disabled={busy || saving || !snapshot.settings.acceleration_enabled} onClick={() => {
                    void invoke("ai_acceleration_disable").then(refresh).catch((e) => setError(String(e)));
                  }}>关闭加速</button>
                </div>
                <div className="chat-download-actions chat-download-maintenance" aria-label="删除加速下载">
                  <button disabled={busy || saving || snapshot.acceleration.busy || !snapshot.acceleration.installed} onClick={() => {
                    void invoke("ai_acceleration_remove", {model: false}).then(refresh).catch((e) => setError(String(e)));
                  }}>删除加速组件</button>
                  <button disabled={busy || saving || snapshot.acceleration.busy || !snapshot.acceleration.model_installed} onClick={() => {
                    void invoke("ai_acceleration_remove", {model: true}).then(refresh).catch((e) => setError(String(e)));
                  }}>删除加速模型</button>
                </div>
                {snapshot.acceleration.status ? <p className="chat-download-status" role="status" data-testid="acceleration-download-status">{snapshot.acceleration.status}</p> : null}
                {snapshot.acceleration_testing ? <button onClick={() => void invoke("ai_acceleration_cancel").then(refresh).catch((e) => setError(String(e)))}>停止试运行</button> : null}
                {snapshot.acceleration.busy ? <>
                  <progress aria-label="加速下载进度" max={snapshot.acceleration.total || 1} value={snapshot.acceleration.received} />
                  <p>{Math.floor(snapshot.acceleration.received / Math.max(1,snapshot.acceleration.total) * 100)}%</p>
                  <button onClick={() => void invoke("ai_acceleration_cancel").then(refresh).catch((e) => setError(String(e)))}>取消加速下载</button>
                </> : null}
              </section>
              <section className="chat-download-section">
                <h2>内置模型与可选下载</h2>
                <p className="chat-footnote">
                  完整版预装2B聊天与视觉权重；轻量版保留CPU运行器和OCR，点击下方下载模型后使用。0.8B仅用于文字聊天，下载后需在上方选择。
                </p>
                <label>
                  本地聊天模型
                  <select
                    disabled={busy || saving}
                    value={snapshot.settings.local_model}
                    onChange={(e) =>
                      void configure({
                        ...snapshot.settings,
                        local_model: e.target.value,
                      })
                    }
                  >
                    <option value="2b">Qwen3.5 2B（默认）</option>
                    <option
                      value="small"
                      disabled={
                        !snapshot.models.some(
                          (m) => m.id === "small" && m.installed,
                        )
                      }
                    >
                      Qwen3.5 0.8B（可选轻量）
                    </option>
                  </select>
                </label>
                {snapshot.models.map((model) => (
                  <div className="chat-model-card" key={model.id}>
                    <strong className="chat-model-name">{model.name}</strong>
                    <span className="chat-model-meta">
                      {(model.bytes / 1024 / 1024).toFixed(0)} MiB ·{" "}
                      {model.installed ? "资源已找到，加载时校验" : "尚未安装"}
                    </span>
                    <p className="chat-footnote">{model.purpose}</p>
                    <div className="chat-setting-actions">
                      <button
                        disabled={snapshot.download.busy}
                        onClick={() =>
                          void invoke("ai_model_download", { id: model.id })
                            .then(refresh)
                            .catch((e) => setError(String(e)))
                        }
                      >
                        {model.installed ? "校验/修复副本" : model.id === "vision" ? "下载视觉组件" : "下载模型"}
                      </button>
                      {model.cached ? (
                        <button
                          disabled={busy || snapshot.download.busy}
                          onClick={() =>
                            void invoke("ai_model_remove", { id: model.id })
                              .then(refresh)
                              .catch((e) => setError(String(e)))
                          }
                        >
                          删除下载副本
                        </button>
                      ) : null}
                    </div>
                  </div>
                ))}
                {snapshot.download.status ? <p className="chat-download-status" role="status">{snapshot.download.status}</p> : null}
                {snapshot.download.busy ? (
                  <>
                    <progress
                      aria-label="模型下载进度"
                      max={snapshot.download.total || 1}
                      value={snapshot.download.received}
                    />
                    <span>
                      {Math.round(
                        (snapshot.download.received /
                          Math.max(1, snapshot.download.total)) *
                          100,
                      )}
                      %
                    </span>
                    <button
                      onClick={() =>
                        void invoke("ai_model_cancel")
                          .then(refresh)
                          .catch((e) => setError(String(e)))
                      }
                    >
                      停止下载
                    </button>
                  </>
                ) : null}
                <p className="chat-footnote">
                  固定来源和版本，校验完成才启用；已下载的同一权重复用。删除副本保留安装包内置资源。
                </p>
              </section>
              <section>
                <h2>模型服务</h2>
                <p className="chat-footnote">
                  当前使用
                  {snapshot.settings.backend === "external"
                    ? "外接服务"
                    : "内置CPU模型"}
                  。API Key保存在系统安全凭据中。
                </p>
                <label>
                  服务地址
                  <input
                    value={serviceUrl}
                    maxLength={512}
                    placeholder="https://服务地址/v1"
                    onChange={(e) => setServiceUrl(e.target.value)}
                  />
                </label>
                <label>
                  模型名
                  <input
                    value={serviceModel}
                    maxLength={128}
                    onChange={(e) => setServiceModel(e.target.value)}
                  />
                </label>
                <label>
                  API Key
                  <input
                    type="password"
                    autoComplete="off"
                    value={apiKey}
                    maxLength={4096}
                    placeholder={
                      snapshot.settings.has_credential
                        ? "已安全保存；留空保留"
                        : "本地服务可不填"
                    }
                    onChange={(e) => setApiKey(e.target.value)}
                  />
                </label>
                <div className="chat-setting-actions">
                  <button
                    disabled={busy || saving || !serviceModel.trim()}
                    onClick={async () => {
                      setSaving(true);
                      setError("");
                      try {
                        await invoke("ai_configure", {
                          settings: {
                            ...snapshot.settings,
                            backend: "external",
                            service_url: serviceUrl.trim(),
                            service_model: serviceModel.trim(),
                            enabled: true,
                          },
                        });
                        if (apiKey)
                          await invoke("ai_save_key", { key: apiKey });
                        setApiKey("");
                        await refresh();
                        setNotice("外接服务已保存，可点击试聊验证连接。");
                      } catch (e) {
                        setError(String(e));
                      } finally {
                        setSaving(false);
                      }
                    }}
                  >
                    保存并使用服务
                  </button>
                  <button
                    disabled={busy || saving}
                    onClick={() =>
                      void configure({
                        ...snapshot.settings,
                        backend: "bundled",
                      })
                    }
                  >
                    使用内置模型
                  </button>
                  <button disabled={!canSend} onClick={() => void send(true)}>
                    测试连接
                  </button>
                  <button
                    disabled={
                      busy || saving || !snapshot.settings.has_credential
                    }
                    onClick={() =>
                      void invoke("ai_save_key", { key: null })
                        .then(refresh)
                        .catch((e) => setError(String(e)))
                    }
                  >
                    删除Key
                  </button>
                </div>
                <h3>屏幕理解服务</h3>
                <p className="chat-footnote">
                  独立于聊天服务。默认在这台电脑内处理屏幕；外接图像模型需单独配置。
                </p>
                <label>
                  屏幕服务地址
                  <input
                    value={screenUrl}
                    maxLength={512}
                    onChange={(e) => setScreenUrl(e.target.value)}
                  />
                </label>
                <label>
                  屏幕模型名
                  <input
                    value={screenModel}
                    maxLength={128}
                    onChange={(e) => setScreenModel(e.target.value)}
                  />
                </label>
                <label>
                  屏幕服务API Key
                  <input
                    type="password"
                    autoComplete="off"
                    value={screenKey}
                    maxLength={4096}
                    placeholder={
                      snapshot.settings.screen_has_credential
                        ? "已安全保存；留空保留"
                        : "本地服务可不填"
                    }
                    onChange={(e) => setScreenKey(e.target.value)}
                  />
                </label>
                <div className="chat-setting-actions">
                  <button
                    disabled={busy || saving || !screenModel.trim()}
                    onClick={async () => {
                      setSaving(true);
                      setError("");
                      try {
                        await invoke("ai_configure", {
                          settings: {
                            ...snapshot.settings,
                            screen_backend: "external",
                            screen_service_url: screenUrl.trim(),
                            screen_service_model: screenModel.trim(),
                          },
                        });
                        if (screenKey)
                          await invoke("ai_save_key", {
                            key: screenKey,
                            screen: true,
                          });
                        setScreenKey("");
                        await refresh();
                        setNotice(
                          "屏幕服务已保存；允许发送后可用理解图片测试。",
                        );
                      } catch (e) {
                        setError(String(e));
                      } finally {
                        setSaving(false);
                      }
                    }}
                  >
                    保存屏幕服务
                  </button>
                  <button
                    disabled={busy || saving}
                    onClick={() =>
                      void configure({
                        ...snapshot.settings,
                        screen_backend: "bundled",
                        remote_screen: false,
                      })
                    }
                  >
                    屏幕使用内置模型
                  </button>
                  <button
                    disabled={
                      busy || saving || !snapshot.settings.screen_has_credential
                    }
                    onClick={() =>
                      void invoke("ai_save_key", { key: null, screen: true })
                        .then(refresh)
                        .catch((e) => setError(String(e)))
                    }
                  >
                    删除屏幕Key
                  </button>
                </div>
                <label className="chat-switch">
                  <input
                    type="checkbox"
                    checked={snapshot.settings.remote_screen}
                    onChange={(e) =>
                      void configure({
                        ...snapshot.settings,
                        remote_screen: e.target.checked,
                      })
                    }
                  />
                  允许向屏幕理解服务发送屏幕文字和图片
                </label>
                <p className="chat-footnote">
                  未开启时，观察内容只交给内置模型。图片兼容性通过“理解图片”单独验证。
                </p>
              </section>
              <section>
                <h2>屏幕观察与主动陪伴</h2>
                <p className="chat-footnote">
                  截图不落盘，观察文字最多在内存保留2分钟。单次观察会等3秒，请切换到目标窗口。
                </p>
                <label className="chat-switch">
                  <input
                    type="checkbox"
                    checked={snapshot.settings.observe_enabled}
                    onChange={(e) =>
                      void configure({
                        ...snapshot.settings,
                        observe_enabled: e.target.checked,
                      })
                    }
                  />
                  自动观察
                </label>
                <label>
                  观察间隔（秒）
                  <input
                    type="number"
                    min={15}
                    max={3600}
                    defaultValue={snapshot.settings.observe_seconds}
                    onBlur={(e) =>
                      void configure({
                        ...snapshot.settings,
                        observe_seconds: Number(e.target.value),
                      })
                    }
                  />
                </label>
                <label className="chat-switch">
                  <input
                    type="checkbox"
                    checked={snapshot.settings.speak_enabled}
                    onChange={(e) =>
                      void configure({
                        ...snapshot.settings,
                        speak_enabled: e.target.checked,
                      })
                    }
                  />
                  偶尔主动说话
                </label>
                <label>
                  主动说话最短间隔（秒）
                  <input
                    type="number"
                    min={60}
                    max={7200}
                    defaultValue={snapshot.settings.speak_seconds}
                    onBlur={(e) =>
                      void configure({
                        ...snapshot.settings,
                        speak_seconds: Number(e.target.value),
                      })
                    }
                  />
                </label>
                <label>
                  观察范围
                  <select
                    value={snapshot.settings.screen_scope}
                    onChange={(e) =>
                      void configure({
                        ...snapshot.settings,
                        screen_scope: e.target.value,
                      })
                    }
                  >
                    <option value="window">当前前台窗口</option>
                    <option value="monitor">整个显示器</option>
                    <option value="all">整个桌面（全部显示器）</option>
                  </select>
                </label>
                {snapshot.settings.screen_scope === "monitor" ? (
                  <label>
                    显示器
                    <select
                      value={snapshot.settings.monitor_id ?? ""}
                      onChange={(e) =>
                        void configure({
                          ...snapshot.settings,
                          monitor_id: e.target.value
                            ? Number(e.target.value)
                            : null,
                        })
                      }
                    >
                      <option value="">主显示器</option>
                      {displays.map((d) => (
                        <option key={d.id} value={d.id}>
                          {d.name}
                          {d.primary ? "（主屏）" : ""}
                        </option>
                      ))}
                    </select>
                  </label>
                ) : null}
                <label>
                  排除应用（用逗号分隔）
                  <input
                    defaultValue={snapshot.settings.excluded_apps.join(", ")}
                    onBlur={(e) =>
                      void configure({
                        ...snapshot.settings,
                        excluded_apps: e.target.value
                          .split(/[,，]/)
                          .map((s) => s.trim())
                          .filter(Boolean)
                          .slice(0, 20),
                      })
                    }
                  />
                </label>
                <label className="chat-switch">
                  <input
                    type="checkbox"
                    checked={snapshot.settings.quiet_hours}
                    onChange={(e) =>
                      void configure({
                        ...snapshot.settings,
                        quiet_hours: e.target.checked,
                      })
                    }
                  />
                  安静时段暂停自动观察与发言
                </label>
                <div className="chat-setting-grid">
                  <label>
                    开始小时
                    <input
                      type="number"
                      min={0}
                      max={23}
                      defaultValue={snapshot.settings.quiet_start}
                      onBlur={(e) =>
                        void configure({
                          ...snapshot.settings,
                          quiet_start: Number(e.target.value),
                        })
                      }
                    />
                  </label>
                  <label>
                    结束小时
                    <input
                      type="number"
                      min={0}
                      max={23}
                      defaultValue={snapshot.settings.quiet_end}
                      onBlur={(e) =>
                        void configure({
                          ...snapshot.settings,
                          quiet_end: Number(e.target.value),
                        })
                      }
                    />
                  </label>
                </div>
                <div className="chat-setting-actions">
                  <button
                    disabled={busy || snapshot.observation.busy}
                    onClick={() =>
                      void invoke("ai_observe", { vision: false })
                        .then(() => {
                          setScreenReply("");
                          setNotice("3秒后观察，请切换到目标窗口。");
                        })
                        .catch((e) => setError(String(e)))
                    }
                  >
                    看看这里（读文字）
                  </button>
                  <button
                    disabled={busy || snapshot.observation.busy}
                    onClick={() =>
                      void invoke("ai_observe", { vision: true })
                        .then(() => {
                          setScreenReply("");
                          setNotice(
                            "3秒后看图，请切换窗口。CPU图片理解可能需要较长时间。",
                          );
                        })
                        .catch((e) => setError(String(e)))
                    }
                  >
                    理解图片
                  </button>
                  <button
                    onClick={() =>
                      void invoke("ai_pause_observation")
                        .then(() => {
                          setScreenReply("");
                          return refresh();
                        })
                        .catch((e) => setError(String(e)))
                    }
                  >
                    暂停并清除观察
                  </button>
                </div>
                <p role="status">{snapshot.observation.status}</p>
                <p className="chat-footnote">
                  本次运行：观察{snapshot.observation.observations}次，主动发言
                  {snapshot.observation.speeches}次。
                </p>
                {snapshot.observation.label ? (
                  <p>{snapshot.observation.label}</p>
                ) : null}
                {snapshot.observation.text ? (
                  <p className="chat-preview-reply">
                    {snapshot.observation.text}
                  </p>
                ) : null}
                {screenReply ? (
                  <p className="chat-preview-reply">{screenReply}</p>
                ) : null}
              </section>
              <section>
                <h2>记住的偏好</h2>
                <p className="chat-footnote">
                  明确告诉大熊的称呼与说话偏好会自动记住。屏幕内容不会成为长期记忆。
                </p>
                <label>
                  搜索记忆
                  <input
                    value={memorySearch}
                    onChange={(e) => setMemorySearch(e.target.value)}
                  />
                </label>
                {snapshot.preferences.length === 0 ? (
                  <p>还没有记住的偏好。</p>
                ) : (
                  snapshot.preferences
                    .filter((p) => `${p.key} ${p.value}`.includes(memorySearch))
                    .map((p) => (
                      <div
                        key={`${p.key}:${p.revision}`}
                        className="chat-memory"
                      >
                        <label>
                          {p.key === "owner_name"
                            ? "对你的称呼"
                            : p.key === "companion_style"
                              ? "陪伴方式"
                              : p.key.startsWith("like:")
                                ? p.key.slice(5)
                                : "说话方式"}
                          {p.key === "speaking_style" ||
                          p.key === "companion_style" ? (
                            <select
                              aria-label={`修改${p.key}`}
                              value={p.value}
                              onChange={(e) =>
                                void invoke("ai_change_preference", {
                                  key: p.key,
                                  value: e.target.value,
                                })
                                  .then(refresh)
                                  .catch((e) => setError(String(e)))
                              }
                            >
                              {(p.key === "speaking_style"
                                ? [
                                    ["brief", "简短"],
                                    ["gentle", "温柔"],
                                    ["playful", "活泼"],
                                  ]
                                : [
                                    ["quiet", "安静陪伴"],
                                    ["normal", "正常陪伴"],
                                  ]
                              ).map(([value, label]) => (
                                <option key={value} value={value}>
                                  {label}
                                </option>
                              ))}
                            </select>
                          ) : (
                            <input
                              aria-label={`修改${p.key}`}
                              defaultValue={p.value}
                              maxLength={64}
                              onBlur={(e) => {
                                if (e.target.value !== p.value)
                                  void invoke("ai_change_preference", {
                                    key: p.key,
                                    value: e.target.value,
                                  })
                                    .then(refresh)
                                    .catch((e) => setError(String(e)));
                              }}
                            />
                          )}
                        </label>
                        <span>
                          {p.origin === "user_chat"
                            ? "你明确告诉大熊的"
                            : "手动编辑"}
                        </span>
                        <button
                          onClick={() =>
                            void invoke("ai_undo_preference", {
                              revision: p.revision,
                            })
                              .then(refresh)
                              .catch((e) => setError(String(e)))
                          }
                        >
                          撤销上次记忆
                        </button>
                        <button
                          onClick={() =>
                            void invoke("ai_change_preference", {
                              key: p.key,
                              value: null,
                            })
                              .then(refresh)
                              .catch((e) => setError(String(e)))
                          }
                        >
                          忘记
                        </button>
                      </div>
                    ))
                )}
              </section>
              <section>
                <h2>个人数据</h2>
                <p className="chat-footnote">
                  清空全部个人数据会删除聊天、长期偏好、性格、AI设置和已保存的服务凭据。宠物游戏记录保留。
                </p>
                {confirmReset ? (
                  <div className="chat-setting-actions">
                    <span>确认删除全部AI个人数据？</span>
                    <button
                      disabled={busy || saving}
                      onClick={async () => {
                        setSaving(true);
                        setError("");
                        try {
                          await invoke("ai_reset_personal_data");
                          setPreview("");
                          setText("");
                          stream.current = null;
                          draftDirty.current = false;
                          initialized.current = false;
                          setScreenReply("");
                          setApiKey("");
                          setScreenKey("");
                          await refresh();
                          setConfirmReset(false);
                          setNotice("AI个人数据已清空，所有观察已关闭。");
                        } catch (e) {
                          setError(String(e));
                        } finally {
                          setSaving(false);
                        }
                      }}
                    >
                      确认删除全部
                    </button>
                    <button onClick={() => setConfirmReset(false)}>取消</button>
                  </div>
                ) : (
                  <button
                    disabled={busy || saving}
                    onClick={() => setConfirmReset(true)}
                  >
                    清空全部AI个人数据
                  </button>
                )}
                <h2>聊天记录</h2>
                <p className="chat-footnote">
                  默认保留最近30天。清空聊天会保留性格设置。
                </p>
                {confirmClear ? (
                  <div className="chat-setting-actions">
                    <span>清空这台电脑上的聊天？</span>
                    <button
                      disabled={busy || saving}
                      onClick={() => void clearChat()}
                    >
                      确认清空
                    </button>
                    <button onClick={() => setConfirmClear(false)}>
                      保留聊天
                    </button>
                  </div>
                ) : (
                  <button
                    disabled={busy || saving}
                    onClick={() => setConfirmClear(true)}
                  >
                    清空聊天
                  </button>
                )}
              </section>
            </div>
          ) : null}
        </>
      )}
    </main>
  );
}
