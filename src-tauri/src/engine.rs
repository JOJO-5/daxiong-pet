//! 行为引擎：每 16ms 被喂一次系统快照，输出「窗口该去哪」「该画哪一帧」「要不要说句话」。
//!
//! 行为优先级（高 → 低）：
//!   番茄钟进行中 > 拖拽中 > 生气中 > 单次反应 > 睡觉 > 物理移动 > 自主漫游 > 注视鼠标 > 待机

use crate::atlas::{self, Row};
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;

// ---- 窗口布局 ----
// 窗口刻意比宠物大：上方留出说话气泡的空间，宠物在窗口内水平居中、贴底。
// 注意：这里的尺寸必须与 src-tauri/tauri.conf.json 的窗口 width / height 一致。
pub const WINDOW_W: i32 = 300;
pub const WINDOW_H: i32 = 240;
pub const PET_W: i32 = 144;
pub const PET_H: i32 = 156;
/// 宠物相对窗口左上角的偏移（水平居中、贴底）
pub const PET_X: i32 = (WINDOW_W - PET_W) / 2;
pub const PET_Y: i32 = WINDOW_H - PET_H;

// ---- 感知与命中 ----
/// 注视感知半径（逻辑像素）：光标进到这个圈里，宠物会转头看它。
const AWARE_RADIUS: f32 = 360.0;
/// 死区半径：光标贴着宠物中心时不判定方向，回落到待机，避免方向乱跳。
const DEAD_RADIUS: f32 = 18.0;
/// 按下后位移超过该值才算拖拽，否则松手时算点击。
const DRAG_THRESHOLD: f32 = 6.0;
/// 命中热区相对「宠物矩形」的内缩比例，避开精灵图四周的透明边缘。
const HOT_INSET_X: f32 = 0.12;
const HOT_INSET_Y: f32 = 0.10;

// ---- 自带漫游 ----
const WANDER_SPEED: f32 = 70.0;
const WANDER_COOLDOWN_MIN: u64 = 8_000;
const WANDER_COOLDOWN_SPAN: u32 = 12_000;

// ---- 说话 ----
/// 多久没被搭理就自言自语一句
const IDLE_TALK_MS: u64 = 45_000;
/// 起身溜达时开口的概率（百分比）
const WANDER_TALK_CHANCE: u32 = 35;

// ---- 摸头 ----
/// 光标在宠物身上静止多久算摸头
const PAT_STILL_MS: u64 = 1_200;
/// 摸头后的冷却，免得一直蹭
const PAT_COOLDOWN_MS: u64 = 6_000;
/// 判定「静止」的位移阈值（逻辑像素）
const PAT_STILL_RADIUS: f32 = 4.0;

// ---- 连点生气 ----
/// 统计窗口：这段时间内点够次数就翻脸
const ANNOY_WINDOW_MS: u64 = 2_000;
const ANNOY_CLICKS: usize = 3;
/// 生气持续多久（期间不搭理你）
const ANNOY_DURATION_MS: u64 = 6_000;

// ---- 睡觉 ----
/// 多久没有任何交互就睡着
const SLEEP_AFTER_MS: u64 = 180_000;

// ---- 物理 ----
/// 重力加速度（像素 / 秒²）
const GRAVITY: f32 = 1500.0;
/// 撞击后的速度保留比例
const BOUNCE: f32 = 0.45;
/// 空中每帧速度衰减（按 60fps 归一）
const AIR_DRAG: f32 = 0.988;
/// 落地后的水平摩擦
const GROUND_FRICTION: f32 = 0.90;
/// 速度上限，防止甩出天际
const MAX_SPEED: f32 = 2600.0;
/// 速度小于此值就归零
const SPEED_EPS: f32 = 2.0;

// ---- 提醒 ----
/// 喝水提醒间隔
const WATER_INTERVAL_MS: u64 = 3_600_000;
/// 番茄钟时长
const POMODORO_MS: u64 = 25 * 60_000;

/// 外部随时可能发来的指令
pub enum Command {
    /// 启动番茄钟
    StartPomodoro,
    /// 取消番茄钟
    CancelPomodoro,
    /// 正在跑就取消，没跑就开始
    TogglePomodoro,
}

/// 触发说话的场合。具体说什么由前端从对应话术表里随机挑。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SayKind {
    Click,
    Drag,
    Idle,
    Wander,
    /// 被摸头
    Pat,
    /// 被连点烦到了
    Annoyed,
    /// 睡着了
    Sleep,
    /// 被吵醒
    Wake,
    /// 喝水提醒
    Water,
    /// 整点报时
    Chime,
    /// 番茄钟开始 / 结束
    PomodoroStart,
    PomodoroEnd,
}

impl SayKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SayKind::Click => "click",
            SayKind::Drag => "drag",
            SayKind::Idle => "idle",
            SayKind::Wander => "wander",
            SayKind::Pat => "pat",
            SayKind::Annoyed => "annoyed",
            SayKind::Sleep => "sleep",
            SayKind::Wake => "wake",
            SayKind::Water => "water",
            SayKind::Chime => "chime",
            SayKind::PomodoroStart => "pomodoro_start",
            SayKind::PomodoroEnd => "pomodoro_end",
        }
    }
}

/// 每 tick 喂进来的系统快照，全部是物理像素坐标。
pub struct Input {
    pub dt_ms: u64,
    /// 隐藏时仍计时，但暂停鼠标交互与窗口运动。
    pub interactive: bool,
    pub cursor: (i32, i32),
    pub win_pos: (i32, i32),
    pub win_size: (i32, i32),
    /// CSS / 逻辑像素到物理像素的缩放比例。
    pub scale_factor: f64,
    /// 当前显示器工作区 (x, y, w, h)
    pub screen: (i32, i32, i32, i32),
    pub button_down: bool,
    /// 当前宠物图集是否带注视行（8x9 的图集没有）
    pub look_enabled: bool,
    /// 重力开关
    pub gravity: bool,
    /// 本地时间的小时数（0-23），用于整点报时
    pub local_hour: u32,
    /// 睡眠时钉住的帧 (行, 列)。契约里没有专门的睡觉动画，
    /// 默认取 failed 行第 2 格（正好是趴着侧躺的姿态）；宠物包可在 pet.json 里覆盖。
    pub sleep_frame: (Row, usize),
}

/// 每 tick 的决策结果。
pub struct Output {
    pub move_to: Option<(i32, i32)>,
    /// 窗口此刻是否应该接收鼠标（false = 点击穿透）
    pub clickable: bool,
    pub row: u8,
    pub col: usize,
    pub say: Option<SayKind>,
    /// 是否处于睡眠状态，前端据此加变暗效果
    pub sleeping: bool,
}

struct Press {
    x: i32,
    y: i32,
    moved: bool,
}

struct Wander {
    /// +1 向右，-1 向左
    dir: f32,
    remain_ms: u64,
}

pub struct Engine {
    // ---- 播放 ----
    row: Row,
    col: usize,
    acc: u64,

    // ---- 注视 ----
    look: Option<u8>,

    // ---- 单次反应 ----
    react: Option<(Row, u64)>,

    // ---- 指针 ----
    press: Option<Press>,
    button_was_down: bool,
    dragging: bool,
    /// 按下时光标相对窗口左上角的偏移，拖拽时保持跟手
    grab: (i32, i32),
    /// 拖拽轨迹采样（用于松手时估算甩出速度）
    trail: VecDeque<(i32, i32, u64)>,

    // ---- 物理 ----
    vx: f32,
    vy: f32,
    /// 重力模式下是否已静止在地面
    grounded: bool,

    // ---- 漫游 ----
    wander: Option<Wander>,
    calm_ms: u64,
    next_wander_ms: u64,

    // ---- 摸头 ----
    pat_ms: u64,
    pat_cd_ms: u64,
    last_cursor: (i32, i32),
    was_hot: bool,

    // ---- 连点 ----
    clicks: VecDeque<u64>,
    annoyed_until_ms: u64,

    // ---- 睡眠 ----
    quiet_ms: u64,
    sleeping: bool,

    // ---- 提醒 ----
    since_water_ms: u64,
    last_hour: Option<u32>,
    pomodoro_ms: Option<u64>,

    // ---- 杂项 ----
    clock_ms: u64,
    since_talk_ms: u64,
    rng: u64,
    cmd_rx: Option<Receiver<Command>>,
}

impl Engine {
    pub fn new() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x2545_F491_4F6C_DD1D)
            | 1;

        Self {
            row: Row::Idle,
            col: 0,
            acc: 0,
            look: None,
            react: None,
            press: None,
            button_was_down: false,
            dragging: false,
            grab: (0, 0),
            trail: VecDeque::new(),
            vx: 0.0,
            vy: 0.0,
            grounded: false,
            wander: None,
            calm_ms: 0,
            next_wander_ms: WANDER_COOLDOWN_MIN,
            pat_ms: 0,
            pat_cd_ms: 0,
            last_cursor: (0, 0),
            was_hot: false,
            clicks: VecDeque::new(),
            annoyed_until_ms: 0,
            quiet_ms: 0,
            sleeping: false,
            since_water_ms: 0,
            last_hour: None,
            pomodoro_ms: None,
            clock_ms: 0,
            since_talk_ms: 0,
            rng: seed,
            cmd_rx: None,
        }
    }

    /// 挂上指令通道，让外部（托盘 / 前端）能驱动番茄钟
    pub fn attach_commands(&mut self, rx: Receiver<Command>) {
        self.cmd_rx = Some(rx);
    }

    /// xorshift64，省掉一个 rand 依赖
    fn rng_next(&mut self) -> u32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 16) as u32
    }

    /// 开始专注：立刻精神起来（running 行由 pomodoro 状态持续驱动）
    fn begin_pomodoro(&mut self, say: &mut Option<SayKind>) {
        if self.pomodoro_ms.is_some() {
            return;
        }
        self.pomodoro_ms = Some(POMODORO_MS);
        *say = Some(SayKind::PomodoroStart);
        self.sleeping = false;
        self.quiet_ms = 0;
    }

    /// 收外部指令。
    /// 先把待处理指令收集出来再执行，避免「持有 channel 引用」与「修改 self」的借用冲突。
    fn drain_commands(&mut self, say: &mut Option<SayKind>) {
        let mut pending = Vec::new();
        if let Some(rx) = &self.cmd_rx {
            while let Ok(cmd) = rx.try_recv() {
                pending.push(cmd);
            }
        }

        for cmd in pending {
            match cmd {
                Command::StartPomodoro => self.begin_pomodoro(say),
                Command::TogglePomodoro => {
                    if self.pomodoro_ms.is_some() {
                        self.pomodoro_ms = None;
                    } else {
                        self.begin_pomodoro(say);
                    }
                }
                Command::CancelPomodoro => self.pomodoro_ms = None,
            }
        }
    }

    pub fn tick(&mut self, input: &Input) -> Output {
        let dt = input.dt_ms;
        // 计时保留真实经过时间，仅物理积分限制步长，避免恢复时飞出屏幕。
        let dt_s = dt.min(120) as f32 / 1000.0;
        self.clock_ms += dt;

        let (cx, cy) = input.cursor;
        let (wx, wy) = input.win_pos;
        let (ww, wh) = input.win_size;
        let scale = input.scale_factor as f32;
        let physical = |value: i32| (value as f32 * scale).round() as i32;
        let (sx, sy, sw, sh) = input.screen;

        let mut say: Option<SayKind> = None;
        self.drain_commands(&mut say);

        // 宠物中心与光标的关系（注视与命中都以宠物为基准，不是整个窗口）
        let center = (wx + physical(PET_X + PET_W / 2), wy + physical(PET_Y + PET_H / 2));
        let dx = (cx - center.0) as f32;
        let dy = (cy - center.1) as f32;
        let dist = (dx * dx + dy * dy).sqrt();

        // ---- 1. 命中热区 ----
        let inset_x = (PET_W as f32 * scale * HOT_INSET_X).round() as i32;
        let inset_y = (PET_H as f32 * scale * HOT_INSET_Y).round() as i32;
        let px = wx + physical(PET_X);
        let py = wy + physical(PET_Y);
        let hot = input.interactive && cx >= px + inset_x
            && cx < px + physical(PET_W) - inset_x
            && cy >= py + inset_y
            && cy < py + physical(PET_H) - inset_y;

        // ---- 2. 注视（睡着 / 番茄钟专注时不看）----
        let busy = self.sleeping || self.pomodoro_ms.is_some();
        self.look = if input.look_enabled && !busy && dist > DEAD_RADIUS * scale && dist < AWARE_RADIUS * scale {
            // atan2(dx, -dy) 把屏幕坐标（y 轴向下）转成 0°=上、90°=右
            let deg = dx.atan2(-dy).to_degrees();
            let deg = if deg < 0.0 { deg + 360.0 } else { deg };
            Some(((deg / 22.5).round() as u32 % 16) as u8)
        } else {
            None
        };

        if !input.interactive {
            self.press = None;
            self.dragging = false;
            self.trail.clear();
            self.wander = None;
            self.vx = 0.0;
            self.vy = 0.0;
        }
        // ---- 3. 按下 / 拖拽 / 点击 ----
        let mut commit_click = false;
        let mut commit_drag = false;
        if input.interactive && input.button_down {
            if !self.button_was_down && self.press.is_none() && hot {
                self.press = Some(Press { x: cx, y: cy, moved: false });
                self.grab = (cx - wx, cy - wy);
                self.trail.clear();
                self.trail.push_back((cx, cy, self.clock_ms));
            }
            let mut started = false;
            if let Some(p) = self.press.as_mut() {
                if !p.moved {
                    let ddx = (cx - p.x) as f32;
                    let ddy = (cy - p.y) as f32;
                    if (ddx * ddx + ddy * ddy).sqrt() > DRAG_THRESHOLD * scale {
                        p.moved = true;
                        started = true;
                    }
                }
            }
            if started {
                self.dragging = true;
                self.wander = None;
                self.vx = 0.0;
                self.vy = 0.0;
                self.grounded = false;
            }
            if self.dragging {
                // 只保留最近 120ms 的轨迹，用来估算松手瞬间的速度
                self.trail.push_back((cx, cy, self.clock_ms));
                while self.trail.len() > 2 {
                    let expired = self
                        .trail
                        .front()
                        .map_or(false, |t| self.clock_ms - t.2 > 120);
                    if expired {
                        self.trail.pop_front();
                    } else {
                        break;
                    }
                }
            }
        } else if let Some(p) = self.press.take() {
            if p.moved {
                commit_drag = true;
            } else {
                commit_click = true;
            }
            self.dragging = false;
        }

        self.button_was_down = input.button_down;

        // ---- 4. 点击：正常反应，或连点翻脸 ----
        if commit_click {
            if self.clock_ms < self.annoyed_until_ms {
                // 还在生气，不理你
            } else {
                self.clicks.push_back(self.clock_ms);
                loop {
                    let expired = self
                        .clicks
                        .front()
                        .map_or(false, |t| self.clock_ms - *t > ANNOY_WINDOW_MS);
                    if expired {
                        self.clicks.pop_front();
                    } else {
                        break;
                    }
                }

                if self.clicks.len() >= ANNOY_CLICKS {
                    self.clicks.clear();
                    self.annoyed_until_ms = self.clock_ms + ANNOY_DURATION_MS;
                    self.react = Some((Row::Failed, 0));
                    say = Some(SayKind::Annoyed);
                } else {
                    let row = if self.rng_next() % 2 == 0 { Row::Waving } else { Row::Jumping };
                    self.react = Some((row, 0));
                    say = Some(SayKind::Click);
                }
            }
        }

        // ---- 5. 拖拽松手：把甩动速度交给物理系统 ----
        if commit_drag {
            if let (Some(first), Some(last)) = (self.trail.front(), self.trail.back()) {
                let span = (last.2 - first.2) as f32 / 1000.0;
                if span > 0.001 {
                    self.vx = ((last.0 - first.0) as f32 / span).clamp(-MAX_SPEED, MAX_SPEED);
                    self.vy = ((last.1 - first.1) as f32 / span).clamp(-MAX_SPEED, MAX_SPEED);
                }
            }
            self.trail.clear();
            self.grounded = false;
            say = Some(SayKind::Drag);
        }

        // ---- 6. 摸头：光标停在宠物身上不动 ----
        let cursor_dx = (cx - self.last_cursor.0) as f32;
        let cursor_dy = (cy - self.last_cursor.1) as f32;
        let cursor_still = (cursor_dx * cursor_dx + cursor_dy * cursor_dy).sqrt() < PAT_STILL_RADIUS * scale;
        self.last_cursor = (cx, cy);

        if hot && cursor_still && !self.dragging {
            self.pat_ms += dt;
        } else {
            self.pat_ms = 0;
        }
        self.pat_cd_ms = self.pat_cd_ms.saturating_sub(dt);

        if self.pat_ms >= PAT_STILL_MS
            && self.pat_cd_ms == 0
            && self.react.is_none()
            && !self.sleeping
            && self.pomodoro_ms.is_none()
        {
            self.pat_ms = 0;
            self.pat_cd_ms = PAT_COOLDOWN_MS;
            self.react = Some((Row::Waving, 0));
            say = Some(SayKind::Pat);
        }

        // ---- 7. 睡眠：长时间无交互就睡，被碰到就醒 ----
        let entered = hot && !self.was_hot;
        let interacted = commit_click || commit_drag || entered;
        self.was_hot = hot;

        if interacted {
            self.quiet_ms = 0;
            if self.sleeping {
                self.sleeping = false;
                self.react = Some((Row::Waving, 0));
                say = Some(SayKind::Wake);
            }
        } else {
            self.quiet_ms += dt;
            if !self.sleeping && self.quiet_ms >= SLEEP_AFTER_MS && self.pomodoro_ms.is_none() {
                self.sleeping = true;
                self.wander = None;
                say = Some(SayKind::Sleep);
            }
        }

        // ---- 8. 物理推进（拖拽中例外，直接跟手）----
        let mut move_to = None;

        if self.dragging {
            move_to = Some((cx - self.grab.0, cy - self.grab.1));
        } else if input.interactive {
            // 漫游时由它接管水平速度
            if let Some(w) = &self.wander {
                self.vx = WANDER_SPEED * w.dir;
            }

            if input.gravity {
                self.vy += GRAVITY * dt_s;
                self.grounded = false;
            }

            let mut nwx = wx + (self.vx * dt_s).round() as i32;
            let mut nwy = wy + (self.vy * dt_s).round() as i32;

            // 左右边界：撞上就反弹
            if nwx < sx {
                nwx = sx;
                self.vx = -self.vx * BOUNCE;
                self.wander = None;
            } else if nwx + ww > sx + sw {
                nwx = sx + sw - ww;
                self.vx = -self.vx * BOUNCE;
                self.wander = None;
            }

            // 上下边界：重力模式下落到「地面」会弹一下，最终停住
            if input.gravity {
                let floor = sy + sh - wh;
                if nwy >= floor {
                    nwy = floor;
                    if self.vy > 120.0 {
                        self.vy = -self.vy * BOUNCE;
                    } else {
                        self.vy = 0.0;
                        self.grounded = true;
                    }
                }
                if nwy < sy {
                    nwy = sy;
                    self.vy = self.vy.abs() * BOUNCE;
                }
            } else if self.vy.abs() > SPEED_EPS {
                // 无重力时也允许被甩出去，但别飞出屏幕
                let top = sy;
                let bottom = sy + sh - wh;
                if nwy < top {
                    nwy = top;
                    self.vy = -self.vy * BOUNCE;
                } else if nwy > bottom {
                    nwy = bottom;
                    self.vy = -self.vy * BOUNCE;
                }
            } else {
                nwy = wy;
            }

            if nwx != wx || nwy != wy {
                move_to = Some((nwx, nwy));
            }

            // 阻尼：落地用摩擦，空中用空气阻力
            let factor = if input.gravity && self.grounded {
                GROUND_FRICTION.powf(dt_s * 60.0)
            } else {
                AIR_DRAG.powf(dt_s * 60.0)
            };
            self.vx *= factor;
            self.vy *= factor;
            if self.vx.abs() < SPEED_EPS {
                self.vx = 0.0;
            }
            if self.vy.abs() < SPEED_EPS && !input.gravity {
                self.vy = 0.0;
            }
        }

        let moving = self.vx.abs() > 8.0 || self.vy.abs() > 8.0;

        // ---- 9. 自主漫游 ----
        let within_aware = dist < AWARE_RADIUS * scale;

        let mut wander_expired = false;
        if let Some(w) = self.wander.as_mut() {
            w.remain_ms = w.remain_ms.saturating_sub(dt);
            wander_expired = w.remain_ms == 0;
        }
        if wander_expired {
            self.wander = None;
        }

        let interrupt = !input.interactive
            || within_aware
            || self.dragging
            || self.press.is_some()
            || self.react.is_some()
            || self.sleeping
            || self.pomodoro_ms.is_some();

        if interrupt {
            self.wander = None;
            self.calm_ms = 0;
        } else if self.wander.is_none() && !moving {
            self.calm_ms += dt;
            if self.calm_ms >= self.next_wander_ms {
                let dir = if self.rng_next() % 2 == 0 { 1.0 } else { -1.0 };
                let dur = 1_500 + (self.rng_next() % 2_500) as u64;
                self.wander = Some(Wander { dir, remain_ms: dur });
                self.calm_ms = 0;
                self.next_wander_ms =
                    WANDER_COOLDOWN_MIN + (self.rng_next() % WANDER_COOLDOWN_SPAN) as u64;
                if say.is_none() && self.rng_next() % 100 < WANDER_TALK_CHANCE {
                    say = Some(SayKind::Wander);
                }
            }
        }

        // ---- 10. 番茄钟倒计时 ----
        if let Some(left) = self.pomodoro_ms.as_mut() {
            if *left <= dt {
                self.pomodoro_ms = None;
                self.react = Some((Row::Jumping, 0));
                say = Some(SayKind::PomodoroEnd);
            } else {
                *left -= dt;
            }
        }

        // ---- 11. 定时提醒（睡着或专注时不打扰）----
        self.since_water_ms += dt;
        // 专注或交互期间暂存到期提醒，空闲后唤醒宠物再提醒。
        if self.since_water_ms >= WATER_INTERVAL_MS
            && self.pomodoro_ms.is_none()
            && self.react.is_none()
            && !self.dragging
            && self.press.is_none()
            && say.is_none()
        {
            self.since_water_ms = 0;
            self.sleeping = false;
            self.quiet_ms = 0;
            self.react = Some((Row::Waving, 0));
            say = Some(SayKind::Water);
        }

        match self.last_hour {
            Some(h) if h != input.local_hour => {
                self.last_hour = Some(input.local_hour);
                if !self.sleeping && self.pomodoro_ms.is_none() && self.react.is_none() {
                    say = Some(SayKind::Chime);
                }
            }
            None => self.last_hour = Some(input.local_hour),
            _ => {}
        }

        // ---- 12. 久无互动就自言自语 ----
        self.since_talk_ms = self.since_talk_ms.saturating_add(dt);
        if self.since_talk_ms >= IDLE_TALK_MS
            && say.is_none()
            && self.react.is_none()
            && !self.dragging
            && !self.sleeping
            && self.wander.is_none()
            && self.pomodoro_ms.is_none()
        {
            say = Some(SayKind::Idle);
        }
        if say.is_some() {
            self.since_talk_ms = 0;
        }

        // ---- 13. 单次反应播完自动收尾 ----
        let mut react_done = false;
        if let Some((r, acc)) = self.react.as_mut() {
            *acc += dt;
            let total: u64 = atlas::track(*r).durations.iter().map(|d| *d as u64).sum();
            if *acc >= total {
                react_done = true;
            }
        }
        if react_done {
            self.react = None;
        }

        // ---- 14. 决定播放哪一行（按优先级）----
        let target = if self.pomodoro_ms.is_some() {
            Row::Running
        } else if self.dragging {
            Row::Waiting
        } else if self.clock_ms < self.annoyed_until_ms && self.react.is_none() {
            Row::Failed
        } else if let Some((r, _)) = self.react {
            r
        } else if self.sleeping {
            input.sleep_frame.0
        } else if self.wander.is_some() {
            if self.vx >= 0.0 { Row::RunRight } else { Row::RunLeft }
        } else if moving {
            if self.vx >= 0.0 { Row::RunRight } else { Row::RunLeft }
        } else if let Some(idx) = self.look {
            if idx < 8 { Row::LookA } else { Row::LookB }
        } else {
            Row::Idle
        };

        if target != self.row {
            self.row = target;
            self.col = 0;
            self.acc = 0;
        }

        // ---- 15. 帧推进 ----
        let track = atlas::track(self.row);
        if self.sleeping && self.react.is_none() && !self.dragging && self.pomodoro_ms.is_none() {
            self.col = input.sleep_frame.1.min(track.cols.saturating_sub(1));
        } else if matches!(self.row, Row::LookA | Row::LookB) {
            if let Some(idx) = self.look {
                self.col = (idx % 8) as usize;
            }
        } else {
            self.acc += dt;
            let dur = track.durations[self.col.min(track.cols - 1)] as u64;
            if self.acc >= dur {
                self.acc = 0;
                if self.col + 1 < track.cols {
                    self.col += 1;
                } else if track.looping {
                    self.col = 0;
                }
            }
        }

        Output {
            move_to,
            // 外部拖拽经过宠物时保持穿透，避免挡住文件投放等操作。
            clickable: self.dragging || (hot && (!input.button_down || self.press.is_some())),
            row: self.row as u8,
            col: self.col,
            say,
            sleeping: self.sleeping,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(scale: f64) -> Input {
        Input {
            dt_ms: 16,
            interactive: true,
            cursor: (-1000, -1000),
            win_pos: (100, 100),
            win_size: ((300.0 * scale) as i32, (240.0 * scale) as i32),
            scale_factor: scale,
            screen: (0, 0, 1920, 1040),
            button_down: false,
            look_enabled: true,
            gravity: false,
            local_hour: 12,
            sleep_frame: (Row::Failed, 2),
        }
    }

    #[test]
    fn hit_area_tracks_display_scaling() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let mut engine = Engine::new();
            let mut i = input(scale);
            i.cursor = (100 + (150.0 * scale) as i32, 100 + (162.0 * scale) as i32);
            assert!(engine.tick(&i).clickable, "scale={scale}");
            i.cursor = (100 + (80.0 * scale) as i32, 100 + (162.0 * scale) as i32);
            assert!(!engine.tick(&i).clickable, "transparent margin at scale={scale}");
        }
    }

    #[test]
    fn external_drag_is_not_captured() {
        let mut engine = Engine::new();
        let mut i = input(1.0);
        i.button_down = true;
        engine.tick(&i);
        i.cursor = (250, 262);
        assert!(!engine.tick(&i).clickable);
        i.cursor.0 += 30;
        assert!(engine.tick(&i).move_to.is_none());
        i.button_down = false;
        assert_ne!(engine.tick(&i).say, Some(SayKind::Click));
        assert!(engine.press.is_none());
    }

    #[test]
    fn pet_drag_still_works_and_scaled_threshold_is_respected() {
        let mut engine = Engine::new();
        let mut i = input(1.5);
        i.cursor = (325, 343);
        i.button_down = true;
        engine.tick(&i);
        i.cursor.0 += 8;
        assert!(engine.tick(&i).move_to.is_none());
        i.cursor.0 += 12;
        assert_eq!(engine.tick(&i).move_to, Some((120, 100)));
        i.button_down = false;
        assert_eq!(engine.tick(&i).say, Some(SayKind::Drag));
    }

    #[test]
    fn sleeping_pet_wakes_for_water_reminder() {
        let mut engine = Engine::new();
        let mut i = input(1.0);
        // 模拟一小时无人互动，使用正常帧间隔。
        i.dt_ms = 100;
        let mut reminders = 0;
        for _ in 0..36_002 {
            let out = engine.tick(&i);
            if out.say == Some(SayKind::Water) {
                reminders += 1;
                assert!(!out.sleeping);
            }
        }
        assert_eq!(reminders, 1);
    }

    #[test]
    fn water_reminder_waits_until_focus_ends() {
        let mut engine = Engine::new();
        let i = input(1.0);
        engine.since_water_ms = WATER_INTERVAL_MS;
        engine.pomodoro_ms = Some(1000);
        assert_ne!(engine.tick(&i).say, Some(SayKind::Water));
        assert!(engine.since_water_ms >= WATER_INTERVAL_MS);
        engine.pomodoro_ms = None;
        assert_eq!(engine.tick(&i).say, Some(SayKind::Water));
    }

    #[test]
    fn floor_uses_physical_window_height_and_work_area() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let mut engine = Engine::new();
            let mut i = input(scale);
            i.gravity = true;
            i.screen = (-1920, 40, 1920, 1000);
            i.win_pos = (-1000, 1000);
            let floor = 1040 - i.win_size.1;
            assert_eq!(engine.tick(&i).move_to, Some((-1000, floor)));
            i.win_pos.1 = floor;
            assert!(engine.tick(&i).move_to.is_none());
        }
    }
    #[test]
    fn long_stall_does_not_extend_pomodoro_or_move_across_screen() {
        let mut engine = Engine::new();
        let mut i = input(1.0);
        engine.pomodoro_ms = Some(60_000);
        engine.vx = 100.0;
        i.dt_ms = 60_000;
        let out = engine.tick(&i);
        assert_eq!(out.say, Some(SayKind::PomodoroEnd));
        assert!(engine.pomodoro_ms.is_none());
        assert_eq!(out.move_to, Some((112, 100)));
    }

    #[test]
    fn sleep_can_pin_a_gaze_frame_without_a_look_direction() {
        let mut engine = Engine::new();
        let mut i = input(1.0);
        engine.sleeping = true;
        i.sleep_frame = (Row::LookB, 7);
        let out = engine.tick(&i);
        assert_eq!((out.row, out.col), (10, 7));
    }

    #[test]
    fn hidden_pet_does_not_interact_or_move_but_timer_finishes() {
        let mut engine = Engine::new();
        let mut i = input(1.0);
        i.interactive = false;
        i.cursor = (250, 262);
        i.button_down = true;
        i.gravity = true;
        engine.pomodoro_ms = Some(1000);
        i.dt_ms = 1000;
        let out = engine.tick(&i);
        assert!(!out.clickable);
        assert!(out.move_to.is_none());
        assert_eq!(out.say, Some(SayKind::PomodoroEnd));
        assert!(engine.press.is_none());
    }

}

/// 根据可见性与睡眠状态降低查询频率；真实计时不依赖轮询频率。
pub fn poll_interval_ms(visible: bool, sleeping: bool) -> u64 {
    if !visible { 500 } else if sleeping { 100 } else { 16 }
}
