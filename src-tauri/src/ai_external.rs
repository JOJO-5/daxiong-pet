//! OpenAI-style optional service. Credentials never enter SQLite or events.
use crate::ai_store::{Result, Settings};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
pub fn validate_url(raw: &str) -> Result<()> {
    let url = reqwest::Url::parse(raw.trim()).map_err(|_| "服务地址无效。")?;
    let local = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    if raw.len() > 512
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !(url.scheme() == "https" || (url.scheme() == "http" && local))
    {
        return Err(
            "服务需使用HTTPS；这台电脑上的服务可使用HTTP。地址不能含凭据或查询参数。".into(),
        );
    }
    Ok(())
}
fn entry(url: &str) -> Result<keyring::Entry> {
    use sha2::{Digest, Sha256};
    keyring::Entry::new(
        "daxiong-pet-ai",
        &format!("{:x}", Sha256::digest(url.trim().as_bytes())),
    )
    .map_err(|_| "系统安全凭据服务不可用。".into())
}
pub fn save_key(url: &str, key: Option<String>) -> Result<()> {
    let e = entry(url)?;
    match key {
        Some(k) if !k.is_empty() && k.len() <= 4096 => e
            .set_password(&k)
            .map_err(|_| "无法保存API Key，请解锁系统凭据服务。".into()),
        Some(k) if !k.is_empty() => Err("API Key过长。".into()),
        _ => match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("无法删除系统凭据。".into()),
        },
    }
}
pub fn key(url: &str) -> Result<String> {
    match entry(url)?.get_password() {
        Ok(k) => Ok(k),
        Err(keyring::Error::NoEntry) => Ok(String::new()),
        Err(_) => Err("无法读取API Key，请解锁系统凭据服务。".into()),
    }
}
async fn cancelable<T>(
    future: impl std::future::Future<Output = T>,
    cancelled: &AtomicBool,
) -> Result<T> {
    tokio::pin!(future);
    loop {
        if cancelled.load(Ordering::SeqCst) {
            return Err("生成已停止。".into());
        }
        tokio::select! {value=&mut future=>return Ok(value),_=tokio::time::sleep(Duration::from_millis(25))=>{}}
    }
}
pub fn generate(
    settings: &Settings,
    mut messages: Vec<Value>,
    output: usize,
    cancelled: &AtomicBool,
    mut on_delta: impl FnMut(&str) -> Result<()>,
) -> Result<String> {
    validate_url(&settings.service_url)?;
    // Image bytes are transport, not text tokens. Reserve a bounded visual budget.
    let budget = |items: &Vec<Value>| -> Result<usize> {
        let mut copy = items.clone();
        let mut images = 0;
        for message in &mut copy {
            if let Some(parts) = message["content"].as_array_mut() {
                for part in parts {
                    if part["type"] == "image_url" {
                        let url = part["image_url"]["url"].as_str().ok_or("图片格式无效。")?;
                        if url.len() > 24 * 1024 * 1024 {
                            return Err("图片过大。".into());
                        }
                        images += 1;
                        part["image_url"]["url"] = json!("[image]");
                    }
                }
            }
        }
        if images > 1 {
            return Err("一次只支持一张图片。".into());
        }
        Ok(serde_json::to_vec(&copy)
            .map_err(|_| "消息格式无效。")?
            .len()
            + images * 2048)
    };
    // Byte-count upper bound avoids assuming the remote tokenizer matches llama.cpp.
    while budget(&messages)? + output + 256 > settings.external_context {
        if messages.len() <= 2 {
            return Err("消息超过外接模型的上下文预算，请缩短或调整设置。".into());
        }
        messages.drain(1..3);
    }
    tauri::async_runtime::block_on(async {
        // Loading the OS trust store can block (especially macOS Keychain).
        // Keep Stop responsive while it is being initialized, too.
        let timeout = settings.timeout_seconds;
        let url = settings.service_url.trim().to_owned();
        let use_system_roots = reqwest::Url::parse(&url)
            .map_err(|_| "服务地址无效。")?
            .scheme()
            == "https";
        let has_credential = settings.has_credential;
        let (client, key) = cancelable(
            tokio::task::spawn_blocking(move || {
                let mut builder = reqwest::Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .connect_timeout(Duration::from_secs(10))
                    .timeout(Duration::from_secs(timeout));
                if !use_system_roots {
                    // validate_url permits HTTP only on loopback, and redirects are disabled.
                    // Local services need neither an HTTPS trust store nor a system proxy.
                    builder = builder.no_proxy().tls_built_in_root_certs(false);
                }
                let client = builder
                    .build()
                    .map_err(|_| "服务连接初始化失败。".to_string())?;
                let credential = if has_credential {
                    key(&url)?
                } else {
                    String::new()
                };
                Ok::<_, String>((client, credential))
            }),
            cancelled,
        )
        .await?
        .map_err(|_| "服务连接初始化失败。")??;
        let mut request=client.post(format!("{}/chat/completions",settings.service_url.trim().trim_end_matches('/'))).json(&json!({"model":settings.service_model,"messages":messages,"max_tokens":output,"stream":true,"temperature":0.6}));
        if !key.is_empty() {
            request = request.bearer_auth(key);
        }
        let response = cancelable(request.send(), cancelled)
            .await?
            .map_err(|_| "外接服务连接失败，请检查地址或网络。")?;
        if !response.status().is_success() {
            return Err(format!(
                "外接服务返回HTTP {}，请检查模型、凭据或服务限额。",
                response.status().as_u16()
            ));
        }
        let stream = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.contains("text/event-stream"));
        if !stream {
            let data = cancelable(response.bytes(), cancelled)
                .await?
                .map_err(|_| "外接服务回复中断。")?;
            if data.len() > 65536 {
                return Err("外接服务返回数据过长。".into());
            }
            let v: Value = serde_json::from_slice(&data).map_err(|_| "外接服务返回格式不兼容。")?;
            let text = v["choices"][0]["message"]["content"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 16384)
                .ok_or("外接服务没有返回文字。")?;
            if cancelled.load(Ordering::SeqCst) {
                return Err("生成已停止。".into());
            }
            on_delta(text)?;
            return Ok(text.into());
        }
        let mut chunks = response.bytes_stream();
        let mut pending = Vec::new();
        let mut reply = String::new();
        let mut finished = false;
        while let Some(chunk) = cancelable(chunks.next(), cancelled).await? {
            pending.extend_from_slice(&chunk.map_err(|_| "外接服务回复中断，已保留收到的文字。")?);
            if pending.len() > 65536 {
                return Err("外接服务单段数据过长。".into());
            }
            while let Some(pos) = pending.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = pending.drain(..=pos).collect();
                let line = std::str::from_utf8(&line)
                    .map_err(|_| "外接服务文字编码无效。")?
                    .trim();
                let Some(data) = line.strip_prefix("data:") else {
                    continue;
                };
                let data = data.trim();
                if data == "[DONE]" {
                    if reply.is_empty() {
                        return Err("外接服务没有返回文字。".into());
                    }
                    return Ok(reply);
                }
                let event: Value =
                    serde_json::from_str(data).map_err(|_| "外接服务流式格式不兼容。")?;
                if event.get("error").is_some() {
                    return Err("外接服务生成失败。".into());
                }
                if let Some(part) = event["choices"][0]["delta"]["content"].as_str() {
                    if reply.len() + part.len() > 16384 {
                        return Err("回复过长，已停止生成。".into());
                    }
                    reply.push_str(part);
                    on_delta(part)?;
                }
                if let Some(reason) = event["choices"][0]["finish_reason"].as_str() {
                    if !["stop", "length"].contains(&reason) {
                        return Err("外接服务未完成文字回复。".into());
                    }
                    finished = true;
                }
            }
        }
        if !finished || reply.is_empty() {
            Err("外接服务回复连接中断。".into())
        } else {
            Ok(reply)
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credential_bearing_and_insecure_remote_urls_are_rejected() {
        for s in [
            "https://user:key@example.com/v1",
            "http://example.com/v1",
            "https://example.com/v1?key=secret",
            "file:///tmp/model",
        ] {
            assert!(validate_url(s).is_err());
        }
        assert!(validate_url("http://127.0.0.1:1234/v1").is_ok());
        assert!(validate_url("https://example.com/v1").is_ok());
    }
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::Arc,
        time::Instant,
    };
    fn mock(
        body: Vec<u8>,
        content_type: &str,
        delay: Duration,
    ) -> (Settings, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let settings = Settings {
            backend: "external".into(),
            service_url: format!("http://{}/v1", listener.local_addr().unwrap()),
            service_model: "test-model".into(),
            ..Settings::default()
        };
        let kind = content_type.to_string();
        let worker = std::thread::spawn(move || {
            listener.set_nonblocking(true).unwrap();
            // Bound fixture lifetime while allowing cold client initialization.
            // The cancellation test may stop before it ever opens a socket.
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => panic!("fixture accept failed: {e}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            // TCP may split headers and JSON across reads. Closing with unread
            // request bytes can reset the connection and discard our response.
            let mut request = Vec::new();
            let mut chunk = [0; 1024];
            loop {
                let n = match stream.read(&mut chunk) {
                    Ok(0) => return, // A cancelled client may leave an incomplete request.
                    Ok(n) => n,
                    Err(e) if matches!(e.kind(), std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => return,
                    Err(e) => panic!("fixture request read failed: {e}"),
                };
                request.extend_from_slice(&chunk[..n]);
                assert!(request.len() <= 8192, "fixture request exceeds limit");
                if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                    let headers = std::str::from_utf8(&request[..end]).unwrap();
                    let length = headers.lines().find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    }).expect("fixture request requires Content-Length");
                    if request.len() >= end + 4 + length { break; }
                }
            }
            std::thread::sleep(delay);
            let header=format!("HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());
            let _ = stream.write_all(header.as_bytes());
            for byte in body {
                if stream.write_all(&[byte]).is_err() {
                    break;
                }
            }
        });
        (settings, worker)
    }
    #[test]
    fn fixture_waits_for_a_split_request_body() {
        use std::net::TcpStream;
        let (settings, worker) = mock(b"ok".to_vec(), "text/plain", Duration::ZERO);
        let url = reqwest::Url::parse(&settings.service_url).unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", url.port().unwrap())).unwrap();
        stream.write_all(b"POST /v1/chat/completions HTTP/1.1\r\nHost: localhost\r\nContent-Length: 4\r\n\r\nab").unwrap();
        stream.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
        let mut byte = [0];
        let error = stream.read(&mut byte).expect_err("fixture replied before receiving the full request");
        assert!(matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut));
        stream.write_all(b"cd").unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with("\r\n\r\nok"));
        worker.join().unwrap();
    }
    #[test]
    fn stream_chunks_and_non_stream_json_are_supported() {
        for(kind,body)in [("text/event-stream","data: {\"choices\":[{\"delta\":{\"content\":\"你好\"}}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n"),("application/json","{\"choices\":[{\"message\":{\"content\":\"你好\"}}]}")] {
            let(settings,worker)=mock(body.as_bytes().to_vec(),kind,Duration::ZERO);let token=AtomicBool::new(false);let mut streamed=String::new();let result=generate(&settings,vec![json!({"role":"user","content":"你好"})],20,&token,|s|{streamed.push_str(s);Ok(())}).unwrap();assert_eq!(result,"你好");assert_eq!(streamed,"你好");worker.join().unwrap();
        }
    }
    #[test]
    fn stalled_remote_request_can_be_cancelled_before_response() {
        let (settings, worker) = mock(Vec::new(), "text/event-stream", Duration::from_millis(500));
        let token = Arc::new(AtomicBool::new(false));
        let cancel = token.clone();
        let timer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            cancel.store(true, Ordering::SeqCst);
        });
        let start = Instant::now();
        assert!(generate(
            &settings,
            vec![json!({"role":"user","content":"你好"})],
            20,
            &token,
            |_| Ok(())
        )
        .is_err());
        assert!(start.elapsed() < Duration::from_millis(450));
        timer.join().unwrap();
        worker.join().unwrap();
    }
}

#[cfg(test)]
mod secure_store_tests {
    use super::*;
    #[test]
    fn optional_native_credential_store_roundtrip() {
        if std::env::var("DAXIONG_TEST_NATIVE_CREDENTIALS").as_deref() != Ok("1") {
            return;
        }
        let url = format!("https://qualification.invalid/{}", uuid::Uuid::new_v4());
        let value = format!("synthetic-{}", uuid::Uuid::new_v4());
        save_key(&url, Some(value.clone())).unwrap();
        let read = key(&url);
        let removed = save_key(&url, None);
        assert!(
            read.is_ok_and(|actual| actual == value),
            "native secure storage roundtrip failed"
        );
        assert!(removed.is_ok(), "native credential cleanup failed");
        assert!(key(&url).unwrap().is_empty());
    }
}
