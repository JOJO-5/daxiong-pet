/**
 * 默认话术表（通用口吻，任何宠物都能用）。
 *
 * 优先级：宠物包自带的 speech.json > 这里。
 * 想给某只宠物定制台词，就在它的宠物包目录里放一个 speech.json：
 *   { "click": ["..."], "drag": ["..."], "idle": ["..."], "wander": ["..."] }
 * 只需写想覆盖的类别，其余自动沿用默认。
 *
 * 文本里的 {time} 会被替换成当前整点（如「14点」）。
 *
 * 各种场合的触发时机由 Rust 侧决定（见 src-tauri/src/engine.rs 的 SayKind）。
 */
export type SpeechTable = Record<string, string[]>;

export const DEFAULT_SPEECH: SpeechTable = {
  // 被点了一下
  click: [
    "在的，我在呢。",
    "被你抓到啦～",
    "有什么要忙的，我盯着呢。",
    "今天的你也很努力哦。",
    "要不要歇一下，喝口水？",
    "陪你干活，我最拿手了。",
    "摸摸头，就不许再摸啦～",
  ],
  // 拖动后放下
  drag: [
    "哇——先放我下来嘛！",
    "别晃啦，我头晕……",
    "要去哪儿呀？我跟着你。",
    "这个高度我有点慌。",
    "轻点轻点，我毛都被你弄乱了。",
  ],
  // 长时间没人理
  idle: [
    "……有点困了。",
    "窗外天气好像不错。",
    "你已经很久没理我了。",
    "发呆也是很重要的事。",
    "要不要站起来伸个懒腰？",
    "我在这儿守着，你放心忙。",
    "好安静啊，只有风扇在转。",
  ],
  // 自己起身溜达
  wander: [
    "我去那边看看～",
    "散散步，活动一下。",
    "换个地方趴着。",
    "巡个逻，马上回来。",
  ],
  // 被摸头
  pat: [
    "嘿嘿，好舒服～",
    "再摸摸嘛，别停。",
    "呼噜呼噜……",
    "就蹭一下下。",
  ],
  // 被连点烦到了
  annoyed: [
    "别戳啦！",
    "哼，不理你了。",
    "我生气了啊。",
    "再点我就走了哦。",
  ],
  // 睡着
  sleep: [
    "Zzz……",
    "有点困，眯一小会儿。",
    "（睡着了）",
  ],
  // 被吵醒
  wake: [
    "唔……醒了醒了。",
    "谁呀，吵到我做梦了。",
    "啊——睡得好香。",
  ],
  // 喝水提醒
  water: [
    "起来喝口水吧，坐挺久了。",
    "该喝水啦，别硬撑。",
    "站起来活动活动腰吧。",
  ],
  // 整点报时
  chime: [
    "叮——{time}啦。",
    "现在是 {time}，又过了一小时。",
    "{time}了，抬头看看远处吧。",
  ],
  // 番茄钟
  pomodoro_start: [
    "专注 25 分钟，开始！我陪着你。",
    "好，进入专注模式，一起加油。",
  ],
  pomodoro_end: [
    "25 分钟到啦，起来走两步！",
    "专注结束，休息一下吧。",
  ],
};

/** 把 {time} 之类的占位符换成实际内容 */
export function formatSpeech(text: string): string {
  const hour = new Date().getHours();
  return text.replace(/\{time\}/g, `${hour}点`);
}

/** 用宠物包自带的话术覆盖默认表（只覆盖写了的类别，空数组忽略） */
export function mergeSpeech(override: unknown): SpeechTable {
  if (!override || typeof override !== "object") return DEFAULT_SPEECH;

  const table: SpeechTable = { ...DEFAULT_SPEECH };
  for (const [kind, list] of Object.entries(override as Record<string, unknown>)) {
    if (!Array.isArray(list)) continue;
    const clean = list.filter(
      (s): s is string => typeof s === "string" && s.trim().length > 0
    );
    if (clean.length > 0) table[kind] = clean;
  }
  return table;
}

/** 按场合随机挑一句，尽量避开刚说过的那句 */
export function pickSpeech(
  table: SpeechTable,
  kind: string,
  last: string | null
): string | null {
  const list = table[kind] ?? table.idle;
  if (!list || list.length === 0) return null;

  let text = list[Math.floor(Math.random() * list.length)];
  for (let i = 0; i < 8 && text === last && list.length > 1; i++) {
    text = list[Math.floor(Math.random() * list.length)];
  }
  return formatSpeech(text);
}
