//! 行为引擎：每 16ms 被喂一次系统快照，输出「窗口该去哪」「该画哪一帧」「要不要说句话」。
//!
//! 行为优先级（高 → 低）：
//!   拖拽中 > 单次互动反应 > 专注姿态 > 生气中 > 睡觉 > 物理移动 > 自主漫游 > 注视鼠标 > 待机

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
    StartTug,
    ShowBall,
    ThrowBall,
    CancelPlay,
    FeedTreat,
    ShowFrisbee,
    ThrowFrisbee,
    DropBall,
    RollBall,
    Trick(crate::activities::Cue,bool),
    PlaceSnack(bool),
    FindSnack,
}

/// 触发说话的场合。具体说什么由前端从对应话术表里随机挑。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SayKind {
    BallInvite,
    PlayReturned,
    TugFinished,
    Click,
    Drag,
    GentleDrag,
    Throw,
    Land,
    Comfort,
    Idle,
    Wander,
    /// 被摸头
    Pat,
    BellyPat,
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
            SayKind::BallInvite => "ball_invite",
            SayKind::PlayReturned => "play_returned",
            SayKind::TugFinished => "tug_finished",
            SayKind::Click => "click",
            SayKind::Drag => "drag",
            SayKind::GentleDrag => "gentle_drag",
            SayKind::Throw => "throw",
            SayKind::Land => "land",
            SayKind::Comfort => "comfort",
            SayKind::Idle => "idle",
            SayKind::Wander => "wander",
            SayKind::Pat => "pat",
            SayKind::BellyPat => "belly_pat",
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
    /// 仅内置大熊使用追加的专属动画；旧包继续使用原动作。
    pub extra_animations: bool,
    pub encounters_enabled: bool,
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
    pub petting: Option<crate::petting::Feedback>,
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
    ambient_reaction: bool,
    next_ambient_ms: u64,

    // ---- 指针 ----
    press: Option<Press>,
    button_was_down: bool,
    was_interactive: bool,
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
    landing_until_ms: u64,

    // ---- 漫游 ----
    wander: Option<Wander>,
    calm_ms: u64,
    next_wander_ms: u64,

    // ---- 摸头 ----
    petting: crate::petting::Petting,
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
    play: crate::play::Play,
    activities: crate::activities::Activities,
    encounters: crate::encounters::Encounters,
    quiet_companion: bool,
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
            ambient_reaction: false,
            next_ambient_ms: 30_000,
            press: None,
            button_was_down: false,
            was_interactive: true,
            dragging: false,
            grab: (0, 0),
            trail: VecDeque::new(),
            vx: 0.0,
            vy: 0.0,
            grounded: false,
            landing_until_ms: 0,
            wander: None,
            calm_ms: 0,
            next_wander_ms: WANDER_COOLDOWN_MIN,
            petting: crate::petting::Petting::default(),
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
            play: crate::play::Play::default(),
            activities: crate::activities::Activities::default(),
            encounters: crate::encounters::Encounters::new(seed),
            quiet_companion: false,
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

    /// 即使连续触发同一行，也从第一帧重新回应。
    fn start_reaction(&mut self, row: Row) {
        self.ambient_reaction = false;
        self.react = Some((row, 0));
        self.row = row;
        self.col = 0;
        self.acc = 0;
    }

    /// 开始专注：立刻精神起来（running 行由 pomodoro 状态持续驱动）
    fn begin_pomodoro(&mut self, say: &mut Option<SayKind>) {
        if self.pomodoro_ms.is_some() {
            return;
        }
        self.play.cancel();self.activities.cancel();
        self.encounters.interrupt();
        self.pomodoro_ms = Some(POMODORO_MS);
        self.react = None;
        self.ambient_reaction = false;
        self.next_ambient_ms = self.clock_ms + 60_000;
        *say = Some(SayKind::PomodoroStart);
        self.sleeping = false;
        self.quiet_ms = 0;
    }

    /// 收外部指令。
    /// 先把待处理指令收集出来再执行，避免「持有 channel 引用」与「修改 self」的借用冲突。
    fn drain_commands(&mut self, input: &Input, say: &mut Option<SayKind>) {
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
                Command::StartTug | Command::ShowBall | Command::ThrowBall | Command::ShowFrisbee | Command::ThrowFrisbee => {
                    if input.interactive && self.pomodoro_ms.is_none() && !self.dragging {
                        self.encounters.interrupt();
                        self.activities.cancel();
                        if matches!(cmd,Command::StartTug) {self.play.start_tug(input);}
                        else if matches!(cmd, Command::ShowFrisbee | Command::ThrowFrisbee) {self.play.start_frisbee(input,matches!(cmd,Command::ThrowFrisbee));}
                        else {self.play.start(input, matches!(cmd, Command::ThrowBall));}
                        self.wander = None; self.react = None; self.sleeping = false;
                        self.quiet_ms = 0; self.vx = 0.0; self.vy = 0.0;
                    }
                }
                Command::PlaceSnack(far) => {
                    if input.interactive && self.pomodoro_ms.is_none() && !self.dragging {
                        self.play.cancel();self.encounters.interrupt();self.activities.place_snack(input,far);
                        self.react=None;self.wander=None;self.sleeping=false;self.quiet_ms=0;self.vx=0.0;self.vy=0.0;
                    } else {self.activities.blocked();}
                }
                Command::FindSnack => self.activities.find_snack(input),
                Command::Trick(cue,learned) => {
                    if input.interactive && self.pomodoro_ms.is_none() && !self.dragging {
                        self.play.cancel();self.encounters.interrupt();self.activities.start(cue,input,learned);
                        self.react=None;self.wander=None;self.sleeping=false;self.quiet_ms=0;self.vx=0.0;self.vy=0.0;
                    } else {self.activities.blocked();}
                }
                Command::RollBall => self.play.roll(input),
                Command::DropBall => self.play.drop_ball(),
                Command::CancelPlay => {self.play.cancel();self.activities.cancel();self.encounters.interrupt();},
                Command::FeedTreat => {
                    self.encounters.interrupt();self.play.cancel();self.activities.cancel();self.sleeping=false;self.quiet_ms=0;
                    self.annoyed_until_ms=0;self.clicks.clear();self.wander=None;
                    self.vx=0.0;self.vy=0.0;
                    self.start_reaction(if input.extra_animations { Row::EatTreat } else { Row::Waving });
                }
            }
        }
    }

    pub fn set_playful_fetch(&mut self,enabled:bool) {self.play.set_playful(enabled);}
    pub fn set_quiet_companion(&mut self,enabled:bool) {
        self.quiet_companion=enabled;
        if enabled {
            self.wander=None;
            if self.ambient_reaction {self.react=None;self.ambient_reaction=false;}
        }
    }
    pub fn is_focusing(&self)->bool {self.pomodoro_ms.is_some()}

    pub fn play_view(&self) -> crate::play::PlayView { self.play.view() }
    pub fn cancel_play(&mut self) { self.play.cancel();self.activities.cancel();self.encounters.interrupt(); }
    pub fn activity_view(&self)->crate::activities::ActivityView {self.activities.view()}
    pub fn encounter_view(&self) -> crate::encounters::EncounterView {self.encounters.view()}

    pub fn tick(&mut self, input: &Input) -> Output {
        let dt = input.dt_ms;
        // 计时保留真实经过时间，仅物理积分限制步长，避免恢复时飞出屏幕。
        let dt_s = dt.min(120) as f32 / 1000.0;
        self.clock_ms += dt;
        // 切换到旧包时取消超出原图集的反应，避免继续输出扩展行。
        if !input.extra_animations && self.react.is_some_and(|(row, _)| row as u8 >= 11) {
            self.react = None;
            self.ambient_reaction = false;
        }

        let (cx, cy) = input.cursor;
        let (wx, wy) = input.win_pos;
        let (ww, wh) = input.win_size;
        let scale = input.scale_factor as f32;
        let physical = |value: i32| (value as f32 * scale).round() as i32;
        let (sx, sy, sw, sh) = input.screen;

        let mut say: Option<SayKind> = None;
        self.drain_commands(input, &mut say);

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
        let hot = input.interactive && !self.activities.pointer_hot(input.cursor,input.scale_factor) && !self.play.pointer_hot(input.cursor,input.scale_factor) && cx >= px + inset_x
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

        // 恢复显示时只同步按键状态，不接管隐藏期间已经开始的拖拽。
        if input.interactive && !self.was_interactive {
            self.button_was_down = input.button_down;
        }
        self.was_interactive = input.interactive;
        if !input.interactive {
            self.play.cancel();self.activities.cancel();
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
                if self.ambient_reaction {
                    self.react = None;
                    self.ambient_reaction = false;
                }
                self.play.cancel();self.activities.cancel();
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
                self.landing_until_ms = 0;
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
                    self.start_reaction(Row::Failed);
                    say = Some(SayKind::Annoyed);
                } else {
                    let choices = if input.extra_animations { 3 } else { 2 };
                    let row = match self.rng_next() % choices {
                        0 => Row::Waving,
                        1 => Row::Jumping,
                        _ => Row::Affection,
                    };
                    self.start_reaction(row);
                    say = Some(SayKind::Click);
                }
            }
        }

        // ---- 5. 拖拽松手：把甩动速度交给物理系统 ----
        if commit_drag {
            // 把松手时的停顿也计入采样：移动后停住再放下不能算甩动。
            self.trail.push_back((cx, cy, self.clock_ms));
            while self.trail.len() > 1 && self.clock_ms - self.trail.front().unwrap().2 > 120 {
                self.trail.pop_front();
            }
            if let (Some(first), Some(last)) = (self.trail.front(), self.trail.back()) {
                let span = (last.2 - first.2) as f32 / 1000.0;
                if span > 0.001 {
                    self.vx = ((last.0 - first.0) as f32 / span).clamp(-MAX_SPEED, MAX_SPEED);
                    self.vy = ((last.1 - first.1) as f32 / span).clamp(-MAX_SPEED, MAX_SPEED);
                }
            }
            self.trail.clear();
            self.grounded = false;
            let thrown = self.vx.hypot(self.vy) / scale > 650.0;
            if thrown {
                self.landing_until_ms = self.clock_ms + 10_000;
                self.start_reaction(Row::Waiting);
                say = Some(SayKind::Throw);
            } else {
                self.landing_until_ms = 0;
                self.start_reaction(Row::Waving);
                say = Some(SayKind::GentleDrag);
            }
        }

        // ---- 6. Light pointer contact; button-down belongs exclusively to dragging.
        let touch = self.petting.tick(dt, (cx-px) as f32 / scale, (cy-py) as f32 / scale,
            input.interactive && !self.play.active() && !self.activities.active()
                && !input.button_down && !self.dragging && !self.sleeping
                && self.pomodoro_ms.is_none() && (self.react.is_none() || self.ambient_reaction)
                && self.vx.abs() <= 8.0 && self.vy.abs() <= 8.0, input.extra_animations);
        let petting = touch.engaged;
        if let Some(kind) = touch.started {
            self.react = None;
            self.ambient_reaction = false;
            let was_annoyed = self.clock_ms < self.annoyed_until_ms;
            self.annoyed_until_ms = 0;
            self.clicks.clear();
            say = Some(match kind {
                crate::petting::Zone::Belly => SayKind::BellyPat,
                crate::petting::Zone::Head if was_annoyed => SayKind::Comfort,
                _ => SayKind::Pat,
            });
        }

        // ---- 7. 睡眠：长时间无交互就睡，被碰到就醒 ----
        let entered = hot && !self.was_hot;
        let interacted = self.activities.active() || (self.play.active() && !self.encounters.offering_ball()) || commit_click || commit_drag || entered || self.dragging || petting;
        self.was_hot = hot;

        if interacted {
            self.quiet_ms = 0;
            if self.sleeping {
                self.sleeping = false;
                self.start_reaction(if input.extra_animations { Row::WakeStretch } else { Row::Waving });
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
        let mut collided = false;

        if self.dragging {
            move_to = Some((cx - self.grab.0, cy - self.grab.1));
        } else if input.interactive && !self.play.active() && !self.activities.active() && !self.encounters.active() {
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
                collided = true;
                self.vx = -self.vx * BOUNCE;
                self.wander = None;
            } else if nwx + ww > sx + sw {
                nwx = sx + sw - ww;
                collided = true;
                self.vx = -self.vx * BOUNCE;
                self.wander = None;
            }

            // 上下边界：重力模式下落到「地面」会弹一下，最终停住
            if input.gravity {
                let floor = sy + sh - wh;
                if nwy >= floor {
                    nwy = floor;
                    collided = true;
                    if self.vy > 120.0 {
                        self.vy = -self.vy * BOUNCE;
                    } else {
                        self.vy = 0.0;
                        self.grounded = true;
                    }
                }
                if nwy < sy {
                    nwy = sy;
                    collided = true;
                    self.vy = self.vy.abs() * BOUNCE;
                }
            } else if self.vy.abs() > SPEED_EPS {
                // 无重力时也允许被甩出去，但别飞出屏幕
                let top = sy;
                let bottom = sy + sh - wh;
                if nwy < top {
                    nwy = top;
                    collided = true;
                    self.vy = -self.vy * BOUNCE;
                } else if nwy > bottom {
                    nwy = bottom;
                    collided = true;
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

        // 扩展动作在停稳后播放，避免还在空中就开始甩毛。
        let settled = self.vx.abs() <= 8.0 && self.vy.abs() <= 8.0
            && (!input.gravity || self.grounded);
        let recovery_due = if input.extra_animations {
            settled && self.react.is_none() && !self.dragging && self.press.is_none()
                && !self.sleeping && self.clock_ms >= self.annoyed_until_ms
        } else {
            collided
        };
        if input.interactive && recovery_due && self.clock_ms < self.landing_until_ms && say.is_none() {
            self.landing_until_ms = 0;
            self.start_reaction(if input.extra_animations { Row::ShakeFur } else { Row::Review });
            say = Some(SayKind::Land);
        }
        if self.clock_ms >= self.landing_until_ms { self.landing_until_ms = 0; }

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

        let interrupt = self.quiet_companion || self.play.active() || self.encounters.active() || !input.interactive
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
                self.start_reaction(Row::Jumping);
                say = Some(SayKind::PomodoroEnd);
            } else {
                *left -= dt;
            }
        }

        // ---- 11. 定时提醒（睡着或专注时不打扰）----
        self.since_water_ms += dt;
        // 专注或交互期间暂存到期提醒，空闲后唤醒宠物再提醒。
        if self.since_water_ms >= WATER_INTERVAL_MS
            && !self.quiet_companion
            && self.pomodoro_ms.is_none()
            && self.react.is_none()
            && !self.dragging
            && self.press.is_none()
            && say.is_none()
        {
            self.since_water_ms = 0;
            self.sleeping = false;
            self.quiet_ms = 0;
            self.start_reaction(Row::Waving);
            say = Some(SayKind::Water);
        }

        match self.last_hour {
            Some(h) if h != input.local_hour => {
                self.last_hour = Some(input.local_hour);
                if !self.quiet_companion && !self.sleeping && self.pomodoro_ms.is_none() && self.react.is_none() {
                    say = Some(SayKind::Chime);
                }
            }
            None => self.last_hour = Some(input.local_hour),
            _ => {}
        }

        // ---- 12. 久无互动就自言自语 ----
        self.since_talk_ms = self.since_talk_ms.saturating_add(dt);
        if self.since_talk_ms >= IDLE_TALK_MS
            && !self.quiet_companion
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

        // 安静的小动作只在可见、静止且没有互动/提醒时出现，不发气泡。
        if !self.quiet_companion && input.interactive && self.clock_ms >= self.next_ambient_ms
            && !self.play.active() && !self.activities.active() && !self.encounters.active() && self.react.is_none() && !self.sleeping && !self.dragging
            && self.press.is_none() && !hot && !moving && self.wander.is_none()
            && self.clock_ms >= self.annoyed_until_ms && say.is_none()
        {
            let row = match self.rng_next() % (if input.extra_animations && self.pomodoro_ms.is_none() { 5 } else { 4 }) {
                0 => Row::Waiting,
                1 => Row::Review,
                2 => Row::Running,
                3 => Row::Waving,
                _ => Row::Affection,
            };
            self.start_reaction(row);
            self.ambient_reaction = true;
            let delay = if self.pomodoro_ms.is_some() { 45_000 } else { 20_000 };
            self.next_ambient_ms = self.clock_ms + delay + (self.rng_next() % 25_000) as u64;
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
            self.ambient_reaction = false;
        }

        let play_phase=self.play.view().phase;
        let hard_blocked=self.quiet_companion || self.activities.active() || !input.interactive || self.sleeping || self.pomodoro_ms.is_some()
            || self.dragging || self.press.is_some() || input.button_down || hot
            || self.clock_ms<self.annoyed_until_ms || (self.react.is_some() && !self.ambient_reaction)
            || (self.play.active() && !(self.encounters.offering_ball() && play_phase=="ready"));
        let encounter=self.encounters.tick(input,hard_blocked,!moving && self.wander.is_none());
        if encounter.cancel_ball && self.play.view().phase=="ready"
            && !(input.button_down && self.play.pointer_hot(input.cursor,input.scale_factor)) { self.play.cancel();self.activities.cancel(); }
        if encounter.offer_ball {
            self.play.invite(input);
            say=Some(SayKind::BallInvite);
        }
        if self.encounters.active() && self.ambient_reaction { self.react=None;self.ambient_reaction=false; }
        if let Some(position)=encounter.movement {
            if self.encounters.offering_ball() { self.play.nudge((position.0-input.win_pos.0,position.1-input.win_pos.1)); }
            move_to=Some(position);
        }
        if let Some(look)=encounter.look { self.look=Some(look); }
        let play_step = self.play.tick(input);
        if let Some(position) = play_step.movement { move_to = Some(position); }
        if play_step.completed {
            let streak=self.play.view().streak;
            self.start_reaction(if input.extra_animations && streak%3==0 {Row::Affection}
                else if input.extra_animations && streak%3==2 {Row::HappyPat} else {Row::Jumping});
            say = Some(if self.play.view().toy=="rope" {SayKind::TugFinished} else {SayKind::PlayReturned});
        }

        let activity=self.activities.tick(input);
        if activity.completed {self.start_reaction(if input.extra_animations {if self.activities.view().game=="snack" {Row::EatTreat} else {Row::HappyPat}} else {Row::Waving});}
        if let Some(position)=activity.movement {move_to=Some(position);}

        // ---- 14. 决定播放哪一行（按优先级）----
        let target = if self.dragging {
            Row::Waiting
        } else if let Some(row)=activity.row {
            row
        } else if let Some(right) = play_step.direction {
            if input.extra_animations && self.play.view().toy=="rope" {
                if right {Row::TugRight} else {Row::TugLeft}
            } else if input.extra_animations && self.play.view().toy=="frisbee" && matches!(self.play.view().phase,"catching" | "returning") {
                if right {Row::DiscRight} else {Row::DiscLeft}
            } else if input.extra_animations && self.play.view().phase=="releasing" {
                if right { Row::DropRight } else { Row::DropLeft }
            } else if input.extra_animations && matches!(self.play.view().phase,"returning" | "teasing") {
                if right { Row::CarryRight } else { Row::CarryLeft }
            } else if right { Row::RunRight } else { Row::RunLeft }
        } else if let Some((row, _)) = touch.frame {
            row
        } else if let Some((r, _)) = self.react {
            r
        } else if let Some(row)=encounter.row {
            row
        } else if self.pomodoro_ms.is_some() {
            Row::Review
        } else if self.clock_ms < self.annoyed_until_ms {
            Row::Failed
        } else if self.sleeping {
            if input.extra_animations { Row::Sleep } else { input.sleep_frame.0 }
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
        if self.sleeping && !input.extra_animations && self.react.is_none() && !self.dragging && self.pomodoro_ms.is_none() {
            self.col = input.sleep_frame.1.min(track.cols.saturating_sub(1));
        } else if matches!(self.row, Row::LookA | Row::LookB) {
            if let Some(idx) = self.look {
                self.col = (idx % 8) as usize;
            }
        } else {
            self.acc += if self.play.view().style=="near" && matches!(self.play.view().phase,"chasing" | "returning") {dt/2} else {dt};
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

        if let Some(col)=activity.col {self.col=col.min(atlas::track(self.row).cols-1);self.acc=0;}
        if matches!(self.row,Row::TugRight|Row::TugLeft) {self.col=self.play.tug_frame().unwrap_or(0);self.acc=0;}
        if let Some((row,col))=touch.frame {if self.row==row {self.col=col;self.acc=0;}}
        self.play.align_carried_ball(input, move_to.unwrap_or(input.win_pos), self.row, self.col);
        if self.quiet_companion && say==Some(SayKind::Sleep) {say=None;}
        Output {
            move_to,
            // 外部拖拽经过宠物时保持穿透，避免挡住文件投放等操作。
            clickable: self.dragging || (hot && (!input.button_down || self.press.is_some())),
            row: self.row as u8,
            col: self.col,
            say,
            sleeping: self.sleeping,
            petting: touch.feedback,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quiet_companion_suppresses_autonomous_interruptions_but_keeps_requested_play() {
        let mut engine=Engine::new();let mut i=input(1.0);i.encounters_enabled=true;i.extra_animations=true;
        engine.set_quiet_companion(true);engine.since_water_ms=WATER_INTERVAL_MS;
        for tick in 0..6000 {
            i.local_hour=12+(tick/3000) as u32;
            let out=engine.tick(&i);
            assert!(out.say.is_none());assert!(engine.wander.is_none());assert!(!engine.encounters.active());
        }
        let (tx,rx)=std::sync::mpsc::channel();engine.attach_commands(rx);
        tx.send(Command::ThrowFrisbee).unwrap();engine.tick(&i);assert_eq!(engine.play_view().phase,"chasing");
        let mut returned=false;
        for _ in 0..3000 {
            let out=engine.tick(&i);if let Some(pos)=out.move_to {i.win_pos=pos;}
            if out.say==Some(SayKind::PlayReturned) {returned=true;break;}
        }
        assert!(returned);assert_eq!(engine.play_view().catches,1);
        tx.send(Command::FeedTreat).unwrap();let out=engine.tick(&i);assert_eq!(out.row,Row::EatTreat as u8);
    }

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
            extra_animations: false,
            encounters_enabled:false,
            gravity: false,
            local_hour: 12,
            sleep_frame: (Row::Failed, 2),
        }
    }

    #[test]
    fn tug_rounds_do_not_reward_fetch_and_new_games_hide_and_focus_cancel_rope() {
        for builtin in [false,true] {
            let (tx,rx)=std::sync::mpsc::channel();let mut e=Engine::new();e.attach_commands(rx);
            let mut i=input(1.25);i.extra_animations=builtin;
            tx.send(Command::StartTug).unwrap();e.tick(&i);
            i.cursor=e.play_view().tug.unwrap().handle;i.button_down=true;e.tick(&i);
            assert_eq!(e.play_view().phase,"tugging");assert!(!e.dragging);
            for _ in 0..120 {
                let rope=e.play_view().tug.unwrap();
                i.cursor=(rope.mouth.0+if rope.right {180} else {-180},rope.mouth.1);
                let out=e.tick(&i);if let Some(pos)=out.move_to {i.win_pos=pos;}
                assert!(!e.dragging);assert!(if builtin {matches!(out.row,23|24)} else {out.row<11});assert_ne!(out.say,Some(SayKind::PlayReturned));
            }
            i.button_down=false;let out=e.tick(&i);
            assert_eq!(out.say,Some(SayKind::TugFinished));assert_eq!(e.play_view().tug_rounds,1);assert_eq!(e.play_view().catches,0);
            tx.send(Command::ShowFrisbee).unwrap();e.tick(&i);assert!(e.play_view().tug.is_none());assert_eq!(e.play_view().toy,"frisbee");
            tx.send(Command::StartTug).unwrap();e.tick(&i);tx.send(Command::PlaceSnack(false)).unwrap();e.tick(&i);assert!(e.play_view().tug.is_none());
            tx.send(Command::StartTug).unwrap();e.tick(&i);i.interactive=false;e.tick(&i);assert!(e.play_view().ball.is_none());assert!(e.play_view().tug.is_none());
            i.interactive=true;tx.send(Command::StartTug).unwrap();e.tick(&i);tx.send(Command::StartPomodoro).unwrap();e.tick(&i);
            assert_eq!(e.play_view().phase,"off");assert!(e.play_view().tug.is_none());
            tx.send(Command::StartTug).unwrap();e.tick(&i);assert_eq!(e.play_view().phase,"off");
        }
    }
    #[test]
    fn dragging_pet_cancels_ready_rope_and_switching_pet_cleans_up() {
        let (tx,rx)=std::sync::mpsc::channel();let mut e=Engine::new();e.attach_commands(rx);let mut i=input(1.0);
        tx.send(Command::StartTug).unwrap();e.tick(&i);
        i.cursor=(i.win_pos.0+PET_X+PET_W/2,i.win_pos.1+PET_Y+PET_H/2);i.button_down=true;e.tick(&i);
        i.cursor.0+=30;e.tick(&i);assert!(e.dragging);assert!(e.play_view().tug.is_none());
        i.button_down=false;e.tick(&i);tx.send(Command::StartTug).unwrap();e.tick(&i);e.cancel_play();assert!(e.play_view().tug.is_none());
    }

    #[test]
    fn frisbee_uses_its_biting_rows_and_focus_cancels() {
        for builtin in [false,true] {
            let (tx,rx)=std::sync::mpsc::channel();let mut e=Engine::new();e.cmd_rx=Some(rx);
            let mut i=input(1.0);i.extra_animations=builtin;tx.send(Command::ThrowFrisbee).unwrap();let mut carried=false;
            for _ in 0..3000 {let out=e.tick(&i);if let Some(p)=out.move_to {i.win_pos=p;}if matches!(e.play_view().phase,"catching"|"returning") {carried=true;assert!(if builtin {matches!(out.row,21|22)} else {out.row<11});}if e.play_view().phase=="returned" {break;}}
            assert!(carried);assert_eq!(e.play_view().phase,"returned");tx.send(Command::ShowFrisbee).unwrap();e.tick(&i);tx.send(Command::StartPomodoro).unwrap();e.tick(&i);assert_eq!(e.play_view().phase,"off");
        }
    }
    #[test]
    fn feeding_plays_every_eating_frame_once_and_legacy_stays_in_bounds() {
        for builtin in [false,true] {
            let (tx,rx)=std::sync::mpsc::channel();let mut e=Engine::new();e.cmd_rx=Some(rx);
            let mut i=input(1.0);i.extra_animations=builtin;
            tx.send(Command::FeedTreat).unwrap();
            let mut cols=std::collections::BTreeSet::new();
            for _ in 0..220 {let out=e.tick(&i);if out.row==Row::EatTreat as u8 {cols.insert(out.col);}if !builtin {assert!(out.row<11);}}
            assert_eq!(cols.len(),if builtin {8} else {0});assert!(e.react.is_none());
            tx.send(Command::FeedTreat).unwrap();e.tick(&i);tx.send(Command::StartPomodoro).unwrap();
            assert_ne!(e.tick(&i).row,Row::EatTreat as u8);
        }
    }
    #[test]
    fn snack_pointer_does_not_drag_pet_and_new_game_clears_cookie() {
        let (tx,rx)=std::sync::mpsc::channel();let mut e=Engine::new();e.cmd_rx=Some(rx);let mut i=input(1.0);
        tx.send(Command::PlaceSnack(false)).unwrap();e.tick(&i);i.cursor=e.activity_view().treat.unwrap();i.button_down=true;e.tick(&i);
        assert_eq!(e.activity_view().phase,"held");assert!(!e.dragging);
        i.button_down=false;e.tick(&i);tx.send(Command::ShowBall).unwrap();e.tick(&i);assert!(e.activity_view().treat.is_none());assert!(e.play.active());
        tx.send(Command::PlaceSnack(true)).unwrap();e.tick(&i);assert!(!e.play.active());assert!(e.activity_view().treat.is_some());
        tx.send(Command::StartPomodoro).unwrap();e.tick(&i);assert!(e.activity_view().treat.is_none());assert!(!e.activities.active());
    }
    #[test]
    fn commands_cancel_ball_and_focus_cancels_tricks_without_rewards() {
        let (tx,rx)=std::sync::mpsc::channel();let mut e=Engine::new();e.cmd_rx=Some(rx);let i=input(1.0);
        tx.send(Command::ShowBall).unwrap();e.tick(&i);assert!(e.play.active());
        tx.send(Command::Trick(crate::activities::Cue::Stay,false)).unwrap();e.tick(&i);assert!(e.activities.active());assert!(!e.play.active());
        tx.send(Command::StartPomodoro).unwrap();e.tick(&i);assert!(!e.activities.active());assert!(!e.activity_view().rewardable);
        tx.send(Command::Trick(crate::activities::Cue::Spin,false)).unwrap();e.tick(&i);assert_eq!(e.activity_view().phase,"blocked");
    }
    #[test]
    fn carrying_uses_biting_frames_only_for_builtin_and_legacy_stays_in_bounds() {
        for builtin in [false,true] {
            let mut engine=Engine::new();let mut i=input(1.0);i.extra_animations=builtin;
            engine.play.start(&i,true);let mut seen=false;let mut released=false;
            for _ in 0..2000 {
                let out=engine.tick(&i);
                if let Some(pos)=out.move_to {i.win_pos=pos;}
                if engine.play_view().phase=="returning" {
                    seen=true;
                    if builtin {assert!(out.row==Row::CarryRight as u8 || out.row==Row::CarryLeft as u8);}
                    else {assert!(out.row<11);}
                }
                if engine.play_view().phase=="releasing" {
                    released=true;assert!(builtin);assert!(out.row==Row::DropRight as u8 || out.row==Row::DropLeft as u8);
                }
                if engine.play_view().phase=="returned" {break;}
            }
            assert!(seen);assert_eq!(released,builtin);assert_eq!(engine.play_view().catches,1);
        }
    }

    #[test]
    fn automatic_events_do_not_keep_an_idle_pet_awake() {
        let mut engine=Engine::new();engine.encounters=crate::encounters::Encounters::new(1);
        let mut i=input(1.0);i.encounters_enabled=true;i.extra_animations=true;i.dt_ms=100;
        let mut seen=false;
        for _ in 0..1805 {
            let out=engine.tick(&i);if let Some(pos)=out.move_to {i.win_pos=pos;}
            seen |= engine.encounter_view().kind.is_some();
        }
        assert!(seen);assert!(engine.sleeping);assert_eq!(engine.encounter_view().kind,None);
        assert_eq!(engine.play_view().phase,"off");
    }

    #[test]
    fn focus_interrupts_an_active_event_and_keeps_original_timer() {
        let mut engine=Engine::new();engine.encounters=crate::encounters::Encounters::new(1);
        let mut i=input(1.0);i.encounters_enabled=true;i.dt_ms=100;
        for _ in 0..1700 {
            let out=engine.tick(&i);if let Some(pos)=out.move_to {i.win_pos=pos;}
            if engine.encounter_view().kind.is_some() {break;}
        }
        assert!(engine.encounter_view().kind.is_some());
        let (tx,rx)=std::sync::mpsc::channel();engine.attach_commands(rx);
        tx.send(Command::StartPomodoro).unwrap();let out=engine.tick(&i);
        assert_eq!(engine.encounter_view().kind,None);assert_eq!(engine.play_view().phase,"off");
        assert_eq!(out.row,Row::Review as u8);assert!(engine.pomodoro_ms.is_some());
    }

    #[test]
    fn builtin_recovery_waits_until_motion_settles() {
        let mut engine = Engine::new();
        let mut i = input(1.0);
        i.extra_animations = true;
        i.win_pos.0 = 1620;
        engine.vx = 1000.0;
        engine.landing_until_ms = 10_000;
        assert_ne!(engine.tick(&i).row, Row::ShakeFur as u8);
        engine.vx = 80.0;
        assert_ne!(engine.tick(&i).row, Row::ShakeFur as u8);
        engine.vx = 0.0;
        let out = engine.tick(&i);
        assert_eq!(out.row, Row::ShakeFur as u8);
        assert_eq!(out.say, Some(SayKind::Land));
        assert_eq!(engine.landing_until_ms, 0);
    }

    #[test]
    fn gravity_recovery_waits_for_ground_contact() {
        let mut engine = Engine::new();
        let mut i = input(1.0);
        i.extra_animations = true;
        i.gravity = true;
        engine.landing_until_ms = 10_000;
        assert_ne!(engine.tick(&i).row, Row::ShakeFur as u8);
        i.win_pos.1 = 800;
        engine.vy = 0.0;
        assert_eq!(engine.tick(&i).row, Row::ShakeFur as u8);
    }

    #[test]
    fn affection_is_reachable_but_not_an_autonomous_focus_action() {
        let mut engine = Engine::new();
        let mut i = input(1.0);
        i.extra_animations = true;
        engine.rng = 1;
        let mut found = false;
        for _ in 0..40 {
            engine.react = None;
            engine.next_ambient_ms = 0;
            found |= engine.tick(&i).row == Row::Affection as u8;
        }
        assert!(found);
        engine.pomodoro_ms = Some(60_000);
        for _ in 0..40 {
            engine.react = None;
            engine.next_ambient_ms = 0;
            assert_ne!(engine.tick(&i).row, Row::Affection as u8);
        }
    }

    #[test]
    fn petting_zones_follow_dpi_and_pressed_contact_still_drags() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let mut e=Engine::new();let mut i=input(scale);i.extra_animations=true;
            i.cursor=(i.win_pos.0+((PET_X+67) as f64*scale).round() as i32,
                i.win_pos.1+((PET_Y+41) as f64*scale).round() as i32);
            let mut starts=0;
            for _ in 0..180 {let out=e.tick(&i);starts+=usize::from(out.say==Some(SayKind::Pat));assert!(out.move_to.is_none());}
            assert_eq!(starts,1);assert_eq!(e.row,Row::HappyPat);
            i.button_down=true;assert!(e.tick(&i).petting.is_none());
            i.cursor.0+=(30.0*scale) as i32;assert_eq!(e.tick(&i).row,Row::Waiting as u8);assert!(e.dragging);
        }
    }

    #[test]
    fn builtin_petting_sleep_and_wake_use_dedicated_rows() {
        let mut engine = Engine::new();
        let mut i = input(1.0);
        i.extra_animations = true;
        i.cursor = (250, 262);
        engine.was_hot = true;
        for _ in 0..80 { engine.tick(&i); }
        assert_eq!(engine.tick(&i).row, Row::HappyPat as u8);
        engine.petting.cancel();
        engine.react = None;
        engine.quiet_ms = SLEEP_AFTER_MS;
        i.cursor = (-1000, -1000);
        assert_eq!(engine.tick(&i).row, Row::Sleep as u8);
        i.dt_ms = 450;
        assert_eq!(engine.tick(&i).col, 1);
        i.dt_ms = 16;
        i.cursor = (250, 262);
        let out = engine.tick(&i);
        assert_eq!(out.say, Some(SayKind::Wake));
        assert_eq!(out.row, Row::WakeStretch as u8);
    }

    #[test]
    fn switching_to_legacy_pet_cancels_extended_reaction() {
        let mut engine = Engine::new();
        engine.start_reaction(Row::WakeStretch);
        let out = engine.tick(&input(1.0));
        assert!(out.row < 11);
        assert!(engine.react.is_none());
    }

    #[test]
    fn thinking_animation_is_reachable_without_speech() {
        let mut engine = Engine::new();
        let i = input(1.0);
        engine.rng = 1;
        let mut found = false;
        for _ in 0..40 {
            engine.react = None;
            engine.next_ambient_ms = 0;
            let out = engine.tick(&i);
            found |= out.row == Row::Running as u8;
            assert!(out.say.is_none());
        }
        assert!(found);
    }

    #[test]
    fn ambient_animation_is_silent_and_interruptible() {
        let mut engine = Engine::new();
        let mut i = input(1.0);
        engine.next_ambient_ms = 0;
        let out = engine.tick(&i);
        assert!(engine.ambient_reaction);
        assert!(out.say.is_none());
        assert!(matches!(engine.row, Row::Waiting | Row::Review | Row::Running | Row::Waving));
        i.cursor = (250, 262);
        i.button_down = true;
        engine.tick(&i);
        assert!(!engine.ambient_reaction);
        assert!(engine.react.is_none());
    }

    #[test]
    fn hidden_sleeping_or_annoyed_pet_skips_ambient_actions() {
        for mode in 0..3 {
            let mut engine = Engine::new();
            let mut i = input(1.0);
            engine.next_ambient_ms = 0;
            match mode {
                0 => i.interactive = false,
                1 => engine.sleeping = true,
                _ => engine.annoyed_until_ms = 6000,
            }
            engine.tick(&i);
            assert!(!engine.ambient_reaction);
        }
    }

    #[test]
    fn focused_pet_can_react_without_stopping_timer() {
        let mut engine = Engine::new();
        let mut i = input(1.0);
        engine.pomodoro_ms = Some(60_000);
        assert_eq!(engine.tick(&i).row, Row::Review as u8);
        i.cursor = (250, 262);
        i.button_down = true;
        engine.tick(&i);
        i.button_down = false;
        let out = engine.tick(&i);
        assert_eq!(out.say, Some(SayKind::Click));
        assert!(matches!(engine.row, Row::Waving | Row::Jumping | Row::Affection));
        assert!(engine.pomodoro_ms.unwrap() < 60_000);
    }

    #[test]
    fn repeated_reaction_restarts_animation() {
        let mut engine = Engine::new();
        engine.start_reaction(Row::Waving);
        engine.col = 3;
        engine.acc = 200;
        engine.start_reaction(Row::Waving);
        let out = engine.tick(&input(1.0));
        assert_eq!((out.row, out.col), (Row::Waving as u8, 0));
    }

    #[test]
    fn petting_comforts_and_prevents_sleep() {
        let mut engine = Engine::new();
        let mut i = input(1.0);
        i.cursor = (250, 262);
        engine.was_hot = true;
        engine.annoyed_until_ms = 6000;
        engine.quiet_ms = SLEEP_AFTER_MS - 1;
        let mut comforted = false;
        for _ in 0..100 {
            comforted |= engine.tick(&i).say == Some(SayKind::Comfort);
            assert!(!engine.sleeping);
        }
        assert!(comforted);
        assert_eq!(engine.annoyed_until_ms, 0);
        assert_eq!(engine.quiet_ms, 0);
    }

    #[test]
    fn release_speed_distinguishes_throw_and_paused_placement() {
        for scale in [1.0, 2.0] {
            for paused in [false, true] {
                let mut engine = Engine::new();
                let mut i = input(scale);
                i.cursor = (100 + (150.0 * scale) as i32, 100 + (162.0 * scale) as i32);
                i.button_down = true;
                engine.tick(&i);
                i.cursor.0 += (80.0 * scale) as i32;
                engine.tick(&i);
                if paused {
                    i.dt_ms = 200;
                    engine.tick(&i);
                }
                i.dt_ms = 16;
                i.button_down = false;
                assert_eq!(engine.tick(&i).say, Some(if paused { SayKind::GentleDrag } else { SayKind::Throw }));
            }
        }
    }

    #[test]
    fn thrown_pet_reacts_to_collision_only_once() {
        let mut engine = Engine::new();
        let mut i = input(1.0);
        i.win_pos.0 = 1620;
        engine.vx = 1000.0;
        engine.landing_until_ms = 5000;
        assert_eq!(engine.tick(&i).say, Some(SayKind::Land));
        engine.vx = 1000.0;
        assert_ne!(engine.tick(&i).say, Some(SayKind::Land));
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
        assert_eq!(engine.tick(&i).say, Some(SayKind::GentleDrag));
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

    #[test]
    fn showing_pet_during_an_external_drag_keeps_click_through() {
        let mut engine = Engine::new();
        let mut i = input(1.0);
        i.interactive = false;
        engine.tick(&i);
        i.interactive = true;
        i.cursor = (250, 262);
        i.button_down = true;
        assert!(!engine.tick(&i).clickable);
        i.cursor.0 += 20;
        assert!(engine.tick(&i).move_to.is_none());
        i.button_down = false;
        engine.tick(&i);
        i.button_down = true;
        engine.tick(&i);
        i.cursor.0 += 20;
        assert!(engine.tick(&i).move_to.is_some());
    }

}

/// 根据可见性与睡眠状态降低查询频率；真实计时不依赖轮询频率。
pub fn poll_interval_ms(visible: bool, sleeping: bool) -> u64 {
    if !visible { 500 } else if sleeping { 100 } else { 16 }
}
