mod ffi;
mod protocol;
use ffi::{Api, Conversation, Engine};
use serde_json::{json, Value};
use std::ffi::c_void;
use std::io::{BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

const REQUEST_SECONDS: u64 = 90;
const CANCEL_SECONDS: u64 = 5;
struct State {
    busy: bool,
    cancelled: bool,
    active: Option<Arc<Conversation>>,
}
struct Control {
    state: Mutex<State>,
    quit: AtomicBool,
    started: Instant,
    request_at: AtomicU64,
    cancel_at: AtomicU64,
}
impl Control {
    fn ms(&self) -> u64 {
        (self.started.elapsed().as_millis() as u64).max(1)
    }
    fn cancel(&self) -> bool {
        let conversation = {
            let mut state = self.state.lock().unwrap();
            if !state.busy {
                return false;
            }
            state.cancelled = true;
            self.cancel_at
                .compare_exchange(0, self.ms(), Ordering::SeqCst, Ordering::SeqCst)
                .ok();
            state.active.clone()
        };
        if let Some(c) = conversation {
            c.cancel();
        }
        true
    }
    fn begin(self: &Arc<Self>) -> Option<Busy> {
        let mut state = self.state.lock().unwrap();
        if state.busy || self.quit.load(Ordering::SeqCst) {
            return None;
        }
        state.busy = true;
        state.cancelled = false;
        self.request_at.store(self.ms(), Ordering::SeqCst);
        Some(Busy(self.clone()))
    }
}
struct Busy(Arc<Control>);
impl Drop for Busy {
    fn drop(&mut self) {
        // Native conversation destruction happens while still busy. A new
        // request cannot overlap this engine's previous conversation cleanup.
        let active = self.0.state.lock().unwrap().active.take();
        drop(active);
        let mut state = self.0.state.lock().unwrap();
        state.busy = false;
        state.cancelled = false;
        self.0.request_at.store(0, Ordering::SeqCst);
        self.0.cancel_at.store(0, Ordering::SeqCst);
    }
}
struct Event {
    raw: String,
    error: Option<String>,
    final_chunk: bool,
}
struct Context {
    api: Arc<Api>,
    sender: mpsc::SyncSender<Event>,
    finished: AtomicBool,
    inflight: AtomicUsize,
    overflow: AtomicBool,
}
unsafe extern "C" fn callback(data: *mut c_void, chunk: *const c_void) {
    let ctx = &*(data as *const Context);
    ctx.inflight.fetch_add(1, Ordering::SeqCst);
    // Never unwind across C. Copy borrowed strings before returning to LiteRT.
    let handled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (raw, error, final_chunk) = ctx.api.chunk(chunk);
        if ctx
            .sender
            .try_send(Event {
                raw,
                error,
                final_chunk,
            })
            .is_err()
        {
            ctx.overflow.store(true, Ordering::SeqCst);
        }
        if final_chunk {
            ctx.finished.store(true, Ordering::SeqCst);
        }
    }));
    if handled.is_err() {
        ctx.overflow.store(true, Ordering::SeqCst);
    }
    ctx.inflight.fetch_sub(1, Ordering::SeqCst); // Last access to callback data.
}
struct LiveStream {
    conversation: Arc<Conversation>,
    context: Box<Context>,
    control: Arc<Control>,
    started: bool,
}
impl Drop for LiveStream {
    fn drop(&mut self) {
        if !self.started {
            return;
        }
        // Final callback + no callback currently using this allocation are
        // required before it can be freed. Deleting a wrapper alone is not a
        // sufficient callback lifetime guarantee.
        if !self.context.finished.load(Ordering::SeqCst) {
            self.control.cancel();
        }
        let deadline = Instant::now() + Duration::from_secs(CANCEL_SECONDS);
        while !self.context.finished.load(Ordering::SeqCst)
            || self.context.inflight.load(Ordering::SeqCst) != 0
        {
            if Instant::now() > deadline {
                eprintln!("LiteRT did not finish cancellation; terminating isolated worker");
                std::process::exit(70);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn response(stream: &mut TcpStream, status: u16, value: Value) -> std::io::Result<()> {
    let body = value.to_string();
    write!(stream,"HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",body.len())?;
    stream.flush()
}
fn error(stream: &mut TcpStream, status: u16, message: &str) {
    let _ = response(stream, status, json!({"error":{"message":message}}));
}
fn sse(stream: &mut TcpStream, value: Value) -> Result<(), String> {
    write!(stream, "data: {}\n\n", value)
        .and_then(|_| stream.flush())
        .map_err(|e| format!("Client disconnected: {e}"))
}
fn peer_closed(stream: &TcpStream) -> bool {
    // Switch only this handler's socket while probing; no concurrent reader or
    // writer uses it. Writes then retain the bounded blocking timeout.
    if stream.set_nonblocking(true).is_err() {
        return true;
    }
    let result = stream.peek(&mut [0u8; 1]);
    let _ = stream.set_nonblocking(false);
    match result {
        Ok(0) => true,
        Err(e) if e.kind() != std::io::ErrorKind::WouldBlock => true,
        _ => false,
    }
}
fn request(stream: &mut TcpStream) -> Result<(String, String, Vec<u8>), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    let mut block = [0u8; 4096];
    let deadline = Instant::now() + Duration::from_secs(5);
    let (header_len, method, path, length) = loop {
        if Instant::now() > deadline {
            return Err("Request read deadline exceeded".into());
        }
        if data.len() > protocol::MAX_BODY + 8192 {
            return Err("Request too large".into());
        }
        let mut headers = [httparse::EMPTY_HEADER; 32];
        let mut parsed = httparse::Request::new(&mut headers);
        if let httparse::Status::Complete(end) = parsed.parse(&data).map_err(|e| e.to_string())? {
            if end > 8192 {
                return Err("Headers too large".into());
            }
            let method = parsed.method.ok_or("Missing method")?.to_string();
            let path = parsed.path.ok_or("Missing path")?.to_string();
            let mut length = None;
            let mut content_type = false;
            for h in parsed.headers.iter() {
                if h.name.eq_ignore_ascii_case("origin")
                    || h.name.eq_ignore_ascii_case("transfer-encoding")
                {
                    return Err("Browser origins and transfer-encoding are not accepted".into());
                }
                if h.name.eq_ignore_ascii_case("content-length") {
                    if length.is_some() {
                        return Err("Duplicate Content-Length".into());
                    }
                    length = Some(
                        std::str::from_utf8(h.value)
                            .map_err(|_| "Bad length")?
                            .parse::<usize>()
                            .map_err(|_| "Bad length")?,
                    );
                }
                if h.name.eq_ignore_ascii_case("content-type") {
                    content_type = h.value.starts_with(b"application/json");
                }
            }
            let length = length.unwrap_or(0);
            if length > protocol::MAX_BODY || (method == "POST" && (!content_type || length == 0)) {
                return Err("A bounded application/json body is required".into());
            }
            break (end, method, path, length);
        }
        if data.len() >= 8192 {
            return Err("Headers too large".into());
        }
        let n = stream.read(&mut block).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("Incomplete headers".into());
        }
        data.extend_from_slice(&block[..n]);
    };
    while data.len() < header_len + length {
        if Instant::now() > deadline {
            return Err("Request read deadline exceeded".into());
        }
        let need = (header_len + length - data.len()).min(block.len());
        let n = stream.read(&mut block[..need]).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("Incomplete body".into());
        }
        data.extend_from_slice(&block[..n]);
    }
    Ok((method, path, data[header_len..header_len + length].to_vec()))
}

fn infer(
    stream: &mut TcpStream,
    engine: &Arc<Engine>,
    api: &Arc<Api>,
    control: &Arc<Control>,
    chat: protocol::Chat,
    id: u64,
) -> Result<(), String> {
    let conversation =
        engine.conversation(&chat.system, &chat.history, chat.output, chat.temperature)?;
    // Verify history wiring using LiteRT's actual rendered preface, independent
    // of a model's recall accuracy. Never expose or log the preface contents.
    let preface = conversation.preface()?;
    let history: Value = serde_json::from_str(&chat.history).map_err(|e| e.to_string())?;
    let messages = history.as_array().ok_or("Invalid normalized history")?;
    for message in messages {
        let text = message["content"][0]["text"]
            .as_str()
            .ok_or("Invalid normalized history text")?;
        if !preface.contains(text) {
            return Err("Requested history missing from native preface".into());
        }
    }
    sse(
        stream,
        json!({"object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":null}],"native_meta":{"history_messages":messages.len(),"history_rendered":true}}),
    )?;
    {
        let mut state = control.state.lock().unwrap();
        state.active = Some(conversation.clone());
        if state.cancelled {
            conversation.cancel();
        }
    }
    if control.state.lock().unwrap().cancelled {
        return Err("Request cancelled before generation".into());
    }
    let (sender, receiver) = mpsc::sync_channel(128);
    let mut live = LiveStream {
        conversation,
        context: Box::new(Context {
            api: api.clone(),
            sender,
            finished: AtomicBool::new(false),
            inflight: AtomicUsize::new(0),
            overflow: AtomicBool::new(false),
        }),
        control: control.clone(),
        started: false,
    };
    let context = &mut *live.context as *mut Context as *mut c_void;
    unsafe {
        live.conversation.stream(&chat.message, callback, context)?;
    }
    live.started = true;
    let begun = Instant::now();
    let mut total = 0;
    loop {
        if live.context.overflow.load(Ordering::SeqCst) {
            return Err("Native callback queue overflow".into());
        }
        if control.quit.load(Ordering::SeqCst) || control.state.lock().unwrap().cancelled {
            return Err("Request cancelled".into());
        }
        if begun.elapsed() > Duration::from_secs(REQUEST_SECONDS) {
            return Err("Inference deadline exceeded".into());
        }
        if peer_closed(stream) {
            return Err("Client disconnected".into());
        }
        match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(event) => {
                if let Some(e) = event.error {
                    return Err(format!(
                        "LiteRT: {}",
                        e.chars().take(400).collect::<String>()
                    ));
                }
                let delta = protocol::delta(&event.raw)?;
                if !delta.is_empty() {
                    total += delta.len();
                    sse(
                        stream,
                        json!({"id":format!("native-{id}"),"object":"chat.completion.chunk","model":"daxiong-litert","choices":[{"index":0,"delta":{"content":delta},"finish_reason":null}]}),
                    )?;
                }
                if event.final_chunk {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return Err("Native stream channel closed".into()),
        }
    }
    if total == 0 {
        return Err("Model returned no text".into());
    }
    sse(
        stream,
        json!({"id":format!("native-{id}"),"object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}),
    )?;
    write!(stream, "data: [DONE]\n\n")
        .and_then(|_| stream.flush())
        .map_err(|e| e.to_string())
}

fn handle(
    mut stream: TcpStream,
    engine: Arc<Engine>,
    api: Arc<Api>,
    control: Arc<Control>,
    serial: Arc<AtomicU64>,
    backend: String,
) {
    let (method, path, body) = match request(&mut stream) {
        Ok(r) => r,
        Err(e) => {
            error(&mut stream, 400, &e);
            return;
        }
    };
    if method == "GET" && path == "/health" {
        let busy = control.state.lock().unwrap().busy;
        let _ = response(
            &mut stream,
            200,
            json!({"ready":true,"busy":busy,"requested_backend":backend,"runtime":"LiteRT-LM 0.18.0","pid":std::process::id()}),
        );
        return;
    }
    if method == "GET" && path == "/v1/models" {
        let _ = response(
            &mut stream,
            200,
            json!({"object":"list","data":[{"id":"daxiong-litert","object":"model"}]}),
        );
        return;
    }
    if method != "POST" || path != "/v1/chat/completions" {
        error(&mut stream, 404, "Unknown endpoint");
        return;
    }
    let chat = match serde_json::from_slice(&body)
        .map_err(|e| e.to_string())
        .and_then(protocol::chat)
    {
        Ok(c) => c,
        Err(e) => {
            error(&mut stream, 400, &e);
            return;
        }
    };
    let Some(_busy) = control.begin() else {
        error(&mut stream, 409, "Worker busy or shutting down");
        return;
    };
    if write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n").and_then(|_|stream.flush()).is_err() { return; }
    let id = serial.fetch_add(1, Ordering::SeqCst);
    if let Err(e) = infer(&mut stream, &engine, &api, &control, chat, id) {
        let _ = sse(
            &mut stream,
            json!({"error":{"message":e,"type":"native_inference_error"}}),
        );
        let _ = write!(stream, "data: [DONE]\n\n");
    }
}

fn event(value: Value) {
    println!("{value}");
    let _ = std::io::stdout().flush();
}
fn run() -> Result<(), String> {
    let mut library = None;
    let mut model = None;
    let mut backend = "cpu".to_string();
    let mut port = 0u16;
    let mut threads = 2;
    let mut context = 1024;
    let mut probe = false;
    let mut args = std::env::args().skip(1);
    while let Some(key) = args.next() {
        match key.as_str() {
            "--library" => library = Some(PathBuf::from(args.next().ok_or("Missing library")?)),
            "--model" => model = Some(PathBuf::from(args.next().ok_or("Missing model")?)),
            "--backend" => backend = args.next().ok_or("Missing backend")?,
            "--port" => {
                port = args
                    .next()
                    .ok_or("Missing port")?
                    .parse()
                    .map_err(|_| "Bad port")?
            }
            "--threads" => {
                threads = args
                    .next()
                    .ok_or("Missing threads")?
                    .parse()
                    .map_err(|_| "Bad threads")?
            }
            "--context" => {
                context = args
                    .next()
                    .ok_or("Missing context")?
                    .parse()
                    .map_err(|_| "Bad context")?
            }
            "--probe" => probe = true,
            "--help" => {
                println!("daxiong-litert-worker --library <absolute SDK library> [--probe | --model <litertlm> --backend cpu|gpu --port 0 --threads 2 --context 1024]\nControl on stdin: {{\"op\":\"cancel\"}} or {{\"op\":\"shutdown\"}}. EOF also shuts down.");
                return Ok(());
            }
            _ => return Err(format!("Unknown argument {key}")),
        }
    }
    if !matches!(backend.as_str(), "cpu" | "gpu")
        || !(1..=16).contains(&threads)
        || !(128..=8192).contains(&context)
    {
        return Err("Invalid backend, threads or context".into());
    }
    let api = Api::load(&library.ok_or("--library is required")?)?;
    if probe {
        event(
            json!({"event":"probe","abi":"LiteRT-LM 0.18.0","native":true,"os":std::env::consts::OS,"arch":std::env::consts::ARCH}),
        );
        return Ok(());
    }
    let model = model
        .ok_or("--model is required")?
        .canonicalize()
        .map_err(|e| format!("Model path: {e}"))?;
    if model.extension().and_then(|v| v.to_str()) != Some("litertlm") {
        return Err("A compatible .litertlm model is required; GGUF is not supported".into());
    }
    let control = Arc::new(Control {
        state: Mutex::new(State {
            busy: false,
            cancelled: false,
            active: None,
        }),
        quit: AtomicBool::new(false),
        started: Instant::now(),
        request_at: AtomicU64::new(0),
        cancel_at: AtomicU64::new(0),
    });
    {
        let c = control.clone();
        std::thread::spawn(move || {
            for line in std::io::stdin().lock().lines() {
                let Ok(line) = line else {
                    break;
                };
                if line.len() > 1024 {
                    continue;
                }
                match serde_json::from_str::<Value>(&line)
                    .ok()
                    .and_then(|v| v.get("op").and_then(Value::as_str).map(str::to_string))
                    .as_deref()
                {
                    Some("cancel") => event(json!({"event":"cancel","accepted":c.cancel()})),
                    Some("shutdown") => break,
                    _ => event(json!({"event":"control_error","message":"Unknown operation"})),
                }
            }
            c.quit.store(true, Ordering::SeqCst);
            c.cancel();
        });
    }
    {
        let c = control.clone();
        std::thread::spawn(move || {
            let mut quit_at = None;
            loop {
                if c.quit.load(Ordering::SeqCst) {
                    if quit_at.is_none() {
                        quit_at = Some(Instant::now());
                    }
                    if quit_at.unwrap().elapsed() > Duration::from_secs(CANCEL_SECONDS) {
                        std::process::exit(70);
                    }
                }
                let cancel = c.cancel_at.load(Ordering::SeqCst);
                let request = c.request_at.load(Ordering::SeqCst);
                let now = c.ms();
                if (cancel != 0 && now.saturating_sub(cancel) > CANCEL_SECONDS * 1000)
                    || (request != 0
                        && now.saturating_sub(request) > (REQUEST_SECONDS + CANCEL_SECONDS) * 1000)
                {
                    std::process::exit(70);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
    }
    let loaded = Instant::now();
    let engine = Engine::create(api.clone(), &model, &backend, threads, context)?;
    if control.quit.load(Ordering::SeqCst) {
        return Ok(());
    }
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    event(
        json!({"event":"ready","port":listener.local_addr().map_err(|e|e.to_string())?.port(),"pid":std::process::id(),"native":true,"requested_backend":backend,"load_seconds":loaded.elapsed().as_secs_f64()}),
    );
    let serial = Arc::new(AtomicU64::new(1));
    let connections = Arc::new(AtomicUsize::new(0));
    let mut handlers = Vec::new();
    while !control.quit.load(Ordering::SeqCst) {
        handlers.retain(|h: &std::thread::JoinHandle<()>| !h.is_finished());
        match listener.accept() {
            Ok((stream, _)) => {
                if connections.load(Ordering::SeqCst) >= 8 {
                    drop(stream);
                    continue;
                }
                connections.fetch_add(1, Ordering::SeqCst);
                let e = engine.clone();
                let a = api.clone();
                let c = control.clone();
                let s = serial.clone();
                let b = backend.clone();
                let n = connections.clone();
                handlers.push(std::thread::spawn(move || {
                    handle(stream, e, a, c, s, b);
                    n.fetch_sub(1, Ordering::SeqCst);
                }));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    drop(listener);
    control.cancel();
    for handle in handlers {
        handle.join().map_err(|_| "Handler panicked")?;
    }
    drop(engine);
    drop(api);
    event(json!({"event":"stopped"}));
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        event(json!({"event":"error","message":e}));
        std::process::exit(1);
    }
}
