//! 精灵图契约。数值来自 codex 宠物 v2 规范，不要随意改动。
//!
//! 图集：1536 x 2288，8 列 x 11 行，单元格 192 x 208，背景透明。
//!
//! 本模块是契约的**完整**映射：图集尺寸常量和 failed / running / review 三个状态行
//! 目前引擎尚未使用（保留它们是为了让契约可读、便于后续扩展），故整体豁免 dead_code。

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
            _ => return None,
        })
    }
}

const D_IDLE: &[u16] = &[280, 110, 110, 140, 140, 320];
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
        Row::Idle => Track { cols: 6, durations: D_IDLE, looping: true },
        Row::RunRight | Row::RunLeft => Track { cols: 8, durations: D_RUN8, looping: true },
        Row::Waving => Track { cols: 4, durations: D_WAVE, looping: false },
        Row::Jumping => Track { cols: 5, durations: D_JUMP, looping: false },
        Row::Failed => Track { cols: 8, durations: D_FAIL, looping: false },
        Row::Waiting => Track { cols: 6, durations: D_WAIT, looping: true },
        Row::Running => Track { cols: 6, durations: D_RUN6, looping: true },
        Row::Review => Track { cols: 6, durations: D_REVIEW, looping: true },
        Row::LookA | Row::LookB => Track { cols: 8, durations: D_LOOK, looping: false },
    }
}
