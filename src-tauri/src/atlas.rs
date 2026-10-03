//! 精灵图契约。数值来自 codex 宠物 v2 规范，不要随意改动。
//!
//! 图集：1536 x 2288，8 列 x 11 行，单元格 192 x 208，背景透明。
//!
//! 前 11 行保留 v2 契约；内置大熊在末尾追加专属动作，旧宠物包保持兼容。

#![allow(dead_code)]

pub const SHEET_W: u32 = 1536;
pub const SHEET_H: u32 = 2288;
pub const CELL_W: u32 = 192;
pub const CELL_H: u32 = 208;

/// 图集行 = 动画状态。数值即行号。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Row {    Idle = 0,
    /// 向右移动（漫游用）
    RunRight = 1,
    /// 向左移动（漫游用）
    RunLeft = 2,
    /// 点击打招呼，单次播放
    Waving = 3,
    /// 点击跳跃，单次播放
    Jumping = 4,
    /// 失败 / 沮丧姿态
    Failed = 5,
    /// 期待姿态：被拖拽时使用
    Waiting = 6,
    /// 忙碌 / 处理中
    Running = 7,
    /// 打量成果
    Review = 8,
    /// 注视方向 000 / 022.5 / 045 / 067.5 / 090 / 112.5 / 135 / 157.5 度
    LookA = 9,
    /// 注视方向 180 / 202.5 / 225 / 247.5 / 270 / 292.5 / 315 / 337.5 度
    LookB = 10,
    /// 内置扩展：摸头开心
    HappyPat = 11,
    /// 内置扩展：睡眠呼吸循环
    Sleep = 12,
    /// 内置扩展：醒来伸懒腰
    WakeStretch = 13,
    /// 内置扩展：甩毛恢复
    ShakeFur = 14,
    /// 内置扩展：歪头抬爪撒娇
    Affection = 15,
    /// Built-in closed-mouth carry cycle.
    CarryRight = 16,
    CarryLeft = 17,
    DropRight = 18,
    DropLeft = 19,
    /// Built-in biscuit taking, chewing and licking sequence.
    EatTreat = 20,
    DiscRight = 21,
    DiscLeft = 22,
    /// Original-identity planted-paw rope bite.
    TugRight = 23,
    TugLeft = 24,
}

/// 一条动画轨道的播放参数。
pub struct Track {
    /// 实际使用的列数（行尾未使用的格子是透明的）
    pub cols: usize,
    /// 每列停留时长（毫秒），长度等于 cols
    pub durations: &'static [u16],
    /// 是否为循环动画；false 表示单次播放后停在最后一帧
    pub looping: bool,
}

impl Row {
    /// 行号 → Row（用于宠物包在 pet.json 里指定睡眠帧之类的场景）
    pub fn from_index(i: u8) -> Option<Row> {
        Some(match i {
            0 => Row::Idle,
            1 => Row::RunRight,
            2 => Row::RunLeft,
            3 => Row::Waving,
            4 => Row::Jumping,
            5 => Row::Failed,
            6 => Row::Waiting,
            7 => Row::Running,
            8 => Row::Review,
            9 => Row::LookA,
            10 => Row::LookB,
            11 => Row::HappyPat,
            12 => Row::Sleep,
            13 => Row::WakeStretch,
            14 => Row::ShakeFur,
            15 => Row::Affection,
            16 => Row::CarryRight,
            17 => Row::CarryLeft,
            18 => Row::DropRight,
            19 => Row::DropLeft,
            20 => Row::EatTreat,
            21 => Row::DiscRight,
            22 => Row::DiscLeft,
            23 => Row::TugRight,
            24 => Row::TugLeft,
            _ => return None,
        })
    }
}

const D_SHAKE: &[u16] = &[180, 110, 110, 110, 110, 110, 140, 260];
const D_AFFECTION: &[u16] = &[180, 200, 220, 220, 300, 220, 180, 260];
const D_HAPPY: &[u16] = &[180, 180, 180, 180, 180, 180, 180, 240];
const D_SLEEP: &[u16] = &[450, 450, 450, 450, 450, 450, 450, 450];
const D_WAKE: &[u16] = &[200, 180, 200, 240, 240, 180, 180, 260];

const D_IDLE: &[u16] = &[280, 110, 110, 140, 140, 320];
const D_EAT: &[u16] = &[450, 350, 350, 300, 300, 300, 350, 400];
const D_DROP: &[u16] = &[100, 100, 100, 100];
const D_RUN8: &[u16] = &[120, 120, 120, 120, 120, 120, 120, 220];
const D_WAVE: &[u16] = &[140, 140, 140, 280];
const D_JUMP: &[u16] = &[140, 140, 140, 140, 280];
const D_FAIL: &[u16] = &[140, 140, 140, 140, 140, 140, 140, 240];
const D_WAIT: &[u16] = &[150, 150, 150, 150, 150, 260];
const D_RUN6: &[u16] = &[120, 120, 120, 120, 120, 220];
const D_REVIEW: &[u16] = &[150, 150, 150, 150, 150, 280];
/// 注视行是静态的：列由角度直接决定，不随时间推进。
const D_LOOK: &[u16] = &[0, 0, 0, 0, 0, 0, 0, 0];

pub fn track(row: Row) -> Track {
    match row {
        Row::TugRight | Row::TugLeft => Track { cols: 8, durations: D_RUN8, looping: true },
        Row::DropRight | Row::DropLeft => Track { cols: 4, durations: D_DROP, looping: false },
        Row::ShakeFur => Track { cols: 8, durations: D_SHAKE, looping: false },
        Row::Affection => Track { cols: 8, durations: D_AFFECTION, looping: false },
        Row::EatTreat => Track { cols: 8, durations: D_EAT, looping: false },
        Row::HappyPat => Track { cols: 8, durations: D_HAPPY, looping: false },
        Row::Sleep => Track { cols: 8, durations: D_SLEEP, looping: true },
        Row::WakeStretch => Track { cols: 8, durations: D_WAKE, looping: false },
        Row::Idle => Track { cols: 6, durations: D_IDLE, looping: true },
        Row::RunRight | Row::RunLeft | Row::CarryRight | Row::CarryLeft | Row::DiscRight | Row::DiscLeft => Track { cols: 8, durations: D_RUN8, looping: true },
        Row::Waving => Track { cols: 4, durations: D_WAVE, looping: false },
        Row::Jumping => Track { cols: 5, durations: D_JUMP, looping: false },
        Row::Failed => Track { cols: 8, durations: D_FAIL, looping: false },
        Row::Waiting => Track { cols: 6, durations: D_WAIT, looping: true },
        Row::Running => Track { cols: 6, durations: D_RUN6, looping: true },
        Row::Review => Track { cols: 6, durations: D_REVIEW, looping: true },
        Row::LookA | Row::LookB => Track { cols: 8, durations: D_LOOK, looping: false },
    }
}
