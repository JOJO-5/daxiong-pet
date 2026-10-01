//! 宠物包发现与解析。
//!
//! 兼容两种形态：
//!
//! 1. **规范包** —— 目录下有 `pet.json` + 图集，且 `spriteVersionNumber == 2`。
//!    标准图集 1536x2288（8 列 x 11 行），含第 9、10 行的 16 方向注视。
//! 2. **裸图集** —— 没有 manifest，只有 `spritesheet.webp` / `.png`。
//!    hatch-pet 的中间产物常见为 1536x1872（8 列 x 9 行），缺注视行；
//!    这种照样接受，只是会关掉注视功能，不至于整个包用不了。
//!
//! 搜索位置：
//! - `${CODEX_HOME:-~/.codex}/pets/` —— 与 codex 本身的宠物目录共用
//! - `<exe 所在目录>/pets/`          —— 绿色版便携位置

use std::path::{Path, PathBuf};

/// 图集契约：固定 8 列，单元格 192x208
pub const COLS: u32 = 8;
pub const CELL_W: u32 = 192;
pub const CELL_H: u32 = 208;
pub const SHEET_W: u32 = CELL_W * COLS;

/// 带注视行的完整行数
pub const ROWS_FULL: u32 = 11;
/// 只有标准动画（无注视）的最小行数
pub const ROWS_MIN: u32 = 9;

/// 默认睡眠帧：failed 行（第 5 行）第 2 格正好是趴着侧躺的姿态。
/// 契约里没有专门的睡觉动画，只能借用它；宠物包可用 pet.json 的 sleepFrame 覆盖。
pub const DEFAULT_SLEEP_ROW: u8 = 5;
pub const DEFAULT_SLEEP_COL: usize = 2;

#[derive(Clone, Debug, serde::Serialize)]
pub struct PetPack {
    pub id: String,
    pub name: String,
    pub description: String,
    /// 图集绝对路径（不序列化给前端，图集内容另行以 data URL 下发）
    #[serde(skip)]
    pub sheet: PathBuf,
    /// 图集行数：11 = 完整，9 = 无注视
    pub rows: u32,
    /// 睡眠时钉住的行号
    pub sleep_row: u8,
    /// 睡眠时钉住的列号
    pub sleep_col: usize,
}

impl PetPack {
    /// 是否包含 16 方向注视行（第 9、10 行）
    pub fn has_look(&self) -> bool {
        self.rows >= ROWS_FULL
    }
}

/// 内置宠物：随程序打包，保证在任何环境下都至少有一只可用
pub fn builtin() -> PetPack {
    PetPack {
        id: "__builtin__".into(),
        name: "大熊".into(),
        description: "内置宠物，随程序打包".into(),
        sheet: PathBuf::new(),
        rows: 16,
        sleep_row: 12,
        sleep_col: 0,
    }
}

/// 待扫描的根目录（不存在的会被跳过）
pub fn search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    let codex_home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(|p| PathBuf::from(p).join(".codex")));
    if let Some(home) = codex_home {
        roots.push(home.join("pets"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.join("pets"));
        }
    }

    roots
}

/// 扫描全部根目录。同一 id 只保留先发现的（codex 目录优先于便携目录）。
pub fn discover() -> Vec<PetPack> {
    let mut found: Vec<PetPack> = vec![builtin()];

    for root in search_roots() {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        // 排序让结果稳定，不随文件系统返回顺序变化
        dirs.sort();

        for dir in dirs {
            if let Some(pack) = load_pack(&dir) {
                if !found.iter().any(|p| p.id == pack.id) {
                    found.push(pack);
                }
            }
        }
    }

    found
}

/// 判定一个目录是不是可用的宠物包
fn load_pack(dir: &Path) -> Option<PetPack> {
    let dir_name = dir.file_name()?.to_string_lossy().to_string();

    // 没有 manifest 时退化成"用目录名当宠物名"
    let mut id = dir_name.clone();
    let mut name = dir_name;
    let mut description = String::new();
    let mut sheet_rel: Option<String> = None;
    let mut sleep_row = DEFAULT_SLEEP_ROW;
    let mut sleep_col = DEFAULT_SLEEP_COL;

    let manifest = dir.join("pet.json");
    if manifest.is_file() {
        let text = std::fs::read_to_string(&manifest).ok()?;
        let value: serde_json::Value = serde_json::from_str(&text).ok()?;

        // 只接受 v2。契约里写明：省略该字段会按 v1 处理并拒收 2288 高的图集。
        let version = value
            .get("spriteVersionNumber")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if version != 2 {
            return None;
        }

        if let Some(s) = value.get("id").and_then(|v| v.as_str()) {
            id = s.to_string();
        }
        if let Some(s) = value.get("displayName").and_then(|v| v.as_str()) {
            name = s.to_string();
        }
        if let Some(s) = value.get("description").and_then(|v| v.as_str()) {
            description = s.to_string();
        }
        if let Some(s) = value.get("spritesheetPath").and_then(|v| v.as_str()) {
            sheet_rel = Some(s.to_string());
        }

        // 可选：自定义睡眠帧。契约里没有睡觉动画，不同宠物趴下的位置可能不同。
        if let Some(sf) = value.get("sleepFrame") {
            if let Some(r) = sf.get("row").and_then(|v| v.as_u64()) {
                sleep_row = u8::try_from(r).unwrap_or(u8::MAX);
            }
            if let Some(c) = sf.get("col").and_then(|v| v.as_u64()) {
                sleep_col = c.min(7) as usize;
            }
        }
    }

    let sheet = match sheet_rel {
        Some(rel) => dir.join(rel),
        None => ["spritesheet.webp", "spritesheet.png"]
            .iter()
            .map(|f| dir.join(f))
            .find(|p| p.is_file())?,
    };
    if !sheet.is_file() {
        return None;
    }

    // 用图片实际尺寸校验契约，避免把不相干的图当作图集
    let (w, h) = image::image_dimensions(&sheet).ok()?;
    if w != SHEET_W || h % CELL_H != 0 {
        return None;
    }
    let rows = h / CELL_H;
    if rows < ROWS_MIN || rows > 256 {
        return None;
    }

    let (sleep_row, sleep_col) = valid_sleep_frame(rows, sleep_row, sleep_col);

    Some(PetPack {
        id,
        name,
        description,
        sheet,
        rows,
        sleep_row,
        sleep_col,
    })
}

/// 把图集读成 data URL 直接下发。
/// 这样无需开启 asset 协议，也不必给任意路径配 scope —— 宠物包放哪都能用。
pub fn sheet_data_url(pack: &PetPack) -> std::io::Result<String> {
    use base64::Engine as _;

    let bytes = std::fs::read(&pack.sheet)?;
    let mime = match pack
        .sheet
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
    {
        Some(ref e) if e == "png" => "image/png",
        _ => "image/webp",
    };

    Ok(format!(
        "data:{};base64,{}",
        mime,
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    ))
}

/// 按实际图集和动画轨道校验睡眠帧；无效行回落到默认睡眠姿态。
fn valid_sleep_frame(rows: u32, row: u8, col: usize) -> (u8, usize) {
    if u32::from(row) >= rows || crate::atlas::Row::from_index(row).is_none() {
        return (DEFAULT_SLEEP_ROW, DEFAULT_SLEEP_COL);
    }
    let track = crate::atlas::track(crate::atlas::Row::from_index(row).unwrap_or(crate::atlas::Row::Failed));
    (row, col.min(track.cols.saturating_sub(1)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sleep_frames_respect_actual_rows_and_used_columns() {
        assert_eq!(valid_sleep_frame(9, 10, 7), (DEFAULT_SLEEP_ROW, DEFAULT_SLEEP_COL));
        assert_eq!(valid_sleep_frame(11, 3, 7), (3, 3));
        assert_eq!(valid_sleep_frame(11, 10, 7), (10, 7));
        assert_eq!(valid_sleep_frame(9, 5, 2), (5, 2));
    }
    #[test]
    fn builtin_is_always_available() {
        let pets = discover();
        assert_eq!(pets.iter().filter(|p| p.id == "__builtin__").count(), 1);
    }
}
