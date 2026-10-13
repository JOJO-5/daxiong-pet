//! One isolated native CPU worker per observation. No screenshot files.
use crate::ai_store::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::Path,
    process::{Child, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
#[derive(Clone, Serialize, Deserialize)]
pub struct Line {
    pub text: String,
    pub confidence: f32,
    pub r#box: Vec<[f32; 2]>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Output {
    pub version: u8,
    pub provider: String,
    pub lines: Vec<Line>,
}
/// Horizontal UI reading order. Anchor each row to its first box so a chain
/// of nearby boxes cannot merge successive lines into a single left-to-right row.
pub fn reading_text(lines: &[Line]) -> String {
    let mut lines: Vec<_> = lines.iter().filter(|l| l.confidence >= 0.65 && l.text.chars().count() > 1).collect();
    lines.sort_by(|a,b| a.r#box[0][1].total_cmp(&b.r#box[0][1]));
    let mut start=0;
    while start<lines.len() {
        let y=lines[start].r#box[0][1];
        let mut end=start+1;
        while end<lines.len() && lines[end].r#box[0][1]-y<10.0 {end+=1;}
        lines[start..end].sort_by(|a,b|a.r#box[0][0].total_cmp(&b.r#box[0][0]));
        start=end;
    }
    lines.iter().map(|l|l.text.as_str()).collect::<Vec<_>>().join("\n").chars().take(1200).collect()
}

#[cfg(test)]
mod reading_tests {
    use super::*;
    #[test]
    fn close_rows_do_not_chain_and_reverse_vertical_reading_order() {
        let line=|text:&str,x,y|Line{text:text.into(),confidence:0.99,r#box:vec![[x,y];4]};
        // Detector output is deliberately unordered; y=8 must not chain y=16 into row 0.
        let lines=[line("第二行",0.,16.),line("第一行右",100.,8.),line("第一行左",0.,0.)];
        assert_eq!(reading_text(&lines),"第一行左\n第一行右\n第二行");
    }
}
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = crate::ai_process::kill(&mut self.0);
        let _ = self.0.wait();
    }
}
pub fn recognize(root: &Path, png: Vec<u8>, threads: u8, cancelled: &AtomicBool) -> Result<Output> {
    if png.is_empty() || png.len() > 16 * 1024 * 1024 || !(1..=4).contains(&threads) {
        return Err("识别图片或CPU设置无效。".into());
    }
    let specs: serde_json::Value = serde_json::from_str(include_str!(
        "../../docs/research/next-ai-integration-2026-10-11/models.json"
    ))
    .map_err(|_| "OCR清单损坏。")?;
    let mut models = Vec::new();
    for key in ["ch_PP-OCRv5_det_mobile", "ch_PP-OCRv5_rec_mobile"] {
        let spec = specs
            .as_array()
            .and_then(|a| a.iter().find(|x| x["key"] == key))
            .ok_or("OCR清单损坏。")?;
        let path = root.join("ocr").join(format!("{key}.onnx"));
        let data = std::fs::read(&path).map_err(|_| "内置OCR模型尚未准备好。")?;
        if data.len() as u64 != spec["bytes"].as_u64().unwrap_or(0)
            || format!("{:x}", Sha256::digest(&data)) != spec["sha256"].as_str().unwrap_or("")
        {
            return Err("OCR模型校验失败。".into());
        }
        models.push(path);
    }
    if cancelled.load(Ordering::SeqCst) {
        return Err("观察已停止。".into());
    }
    let runtime = root.join("ocr");
    let binary = runtime.join(if cfg!(windows) {
        "daxiong-ocr.exe"
    } else {
        "daxiong-ocr"
    });
    let mut command = crate::ai_process::command(&binary, &root.join("runtime"))?;
    command
        .arg(&models[0])
        .arg(&models[1])
        .arg(threads.to_string())
        .current_dir(&runtime)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() == 1 {
                    return Err(std::io::Error::other("parent exited"));
                }
                Ok(())
            });
        }
    }
    let mut child = OwnedChild(
        command
            .spawn()
            .and_then(crate::ai_process::own)
            .map_err(|_| "无法启动OCR，请检查内置资源。")?,
    );
    let mut input = child.0.stdin.take().ok_or("OCR输入不可用。")?;
    let output = child.0.stdout.take().ok_or("OCR输出不可用。")?;
    let (tx, rx) = mpsc::channel();
    let writer = std::thread::spawn(move || input.write_all(&png));
    let reader = std::thread::spawn(move || {
        let mut data = Vec::new();
        let result = output.take(512 * 1024 + 1).read_to_end(&mut data);
        let _ = tx.send((result, data));
    });
    let start = Instant::now();
    let status = loop {
        if cancelled.load(Ordering::SeqCst) || start.elapsed() > Duration::from_secs(30) {
            let _ = crate::ai_process::kill(&mut child.0);
            let _ = child.0.wait();
            let _ = writer.join();
            let _ = reader.join();
            return Err(if cancelled.load(Ordering::SeqCst) {
                "观察已停止。"
            } else {
                "OCR识别超时，已释放进程。"
            }
            .into());
        }
        if let Some(status) = child.0.try_wait().map_err(|_| "OCR状态不可用。")? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let write = writer.join().map_err(|_| "OCR图片发送失败。")?;
    let _ = reader.join();
    let (read, data) = rx.recv().map_err(|_| "OCR返回中断。")?;
    if !status.success() || write.is_err() || read.is_err() || data.len() > 512 * 1024 {
        return Err("OCR识别失败，桌宠可以继续使用。".into());
    }
    let result: Output = serde_json::from_slice(&data).map_err(|_| "OCR输出格式无效。")?;
    if result.version != 1
        || result.provider != "cpu"
        || result.lines.len() > 128
        || result.lines.iter().any(|l| {
            l.text.chars().count() > 2000
                || !l.confidence.is_finite()
                || !(0.0..=1.0).contains(&l.confidence)
                || l.r#box.len() != 4
                || l.r#box.iter().flatten().any(|v| !v.is_finite())
        })
    {
        return Err("OCR结果超出允许范围。".into());
    }
    Ok(result)
}
