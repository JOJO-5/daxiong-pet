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
  play_returned: ["接住啦，再扔一次嘛！", "叼回来了！我的球技怎么样？", "接球可是我的拿手项目。"],
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
  gentle_drag: ["稳稳落地，谢谢你。", "这个位置不错，就待这里啦。"],
  throw: ["哇——飞起来了！", "慢一点，我还没准备好！"],
  land: ["呼，站稳啦。", "甩甩毛，继续陪你。"],
  comfort: ["好啦，原谅你了。", "摸摸头就和好啦。"],
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

/** 内置大熊的边牧口吻；外部宠物仍使用通用默认表。 */
export const DAXIONG_SPEECH: SpeechTable = {
  ...DEFAULT_SPEECH,
  click: ["汪！大熊收到。", "叫我了吗？耳朵已经竖起来啦。", "桌面巡逻员，随时待命！", "抓到一只认真陪你的边牧。", "给你一个爪爪。", "你忙你的，我负责陪着。"],
  gentle_drag: ["四只爪爪都站稳啦。", "新岗哨不错，我就在这儿守着。", "轻轻放下，毛毛还是整齐的。", "搬家完毕，继续陪你。"],
  throw: ["汪——这趟没有安全带呀！", "耳朵要被风吹起来啦！", "我会跑，可没学过飞呀！", "爪爪准备，找个地方落地！"],
  land: ["甩甩毛，巡逻员重新上线！", "四只爪爪，安全着陆。", "呼，桌面还是踩着舒服。", "站稳了，耳朵也归位啦。"],
  comfort: ["好吧，爪爪给你，我们和好。", "摸摸头，气就跑掉啦。", "原谅你了，再轻一点哦。", "尾巴已经偷偷摇起来了。"],
  pat: ["耳朵后面也摸摸嘛。", "尾巴藏不住开心啦。", "这只边牧已经被你摸得软乎乎了。", "再摸一下，就一下。", "爪爪搭好，继续陪你。"],
  annoyed: ["鼻子不是按钮啦！", "汪，轻一点嘛。", "我先收起爪爪，等你温柔一点。", "耳朵都被你戳耷拉啦。"],
  idle: ["桌面没有羊，那就守着你吧。", "耳朵值班中，你安心忙。", "我刚刚检查过，这片桌面很安全。", "想跑两步，又舍不得离你太远。", "有球的话，我肯定第一个发现。", "趴一会儿，耳朵还在听。"],
  wander: ["巡逻一小圈，很快回来。", "那边好像有动静，我去看看。", "爪爪活动一下。", "边牧巡逻员换个岗哨。"],
  sleep: ["巡逻员休息一会儿……Zzz。", "梦里有一大片草地。", "爪爪收好，眯一会儿。"],
  wake: ["汪？我没偷懒，只是闭眼值班。", "耳朵上线！你叫我啦？", "梦里的球还没叼回来呢。"],
  pomodoro_start: ["你专心，我安静站岗。25 分钟，开始！", "大熊陪你专注一会儿，爪爪不乱跑。"],
  pomodoro_end: ["汪！25 分钟完成，给你一个开心跳。", "任务完成！一起伸个懒腰吧。"],
};

export type SpeechHistory = Map<string, string[]>;

/** 把 {time} 之类的占位符换成实际内容 */
export function formatSpeech(text: string): string {
  const hour = new Date().getHours();
  return text.replace(/\{time\}/g, `${hour}点`);
}

/** 用宠物包自带的话术覆盖默认表（只覆盖写了的类别，空数组忽略） */
export function mergeSpeech(override: unknown, base: SpeechTable = DEFAULT_SPEECH): SpeechTable {
  if (!override || typeof override !== "object") return base;

  const table: SpeechTable = { ...base };
  for (const [kind, list] of Object.entries(override as Record<string, unknown>)) {
    if (!Array.isArray(list)) continue;
    const clean = list.filter(
      (s): s is string => typeof s === "string" && s.trim().length > 0
    );
    if (clean.length > 0) table[kind] = clean;
  }
  for (const [kind, parent] of Object.entries({ gentle_drag: "drag", throw: "drag", land: "drag", comfort: "pat" })) {
    if (!(kind in (override as object)) && parent in (override as object)) table[kind] = table[parent];
  }
  return table;
}

/** 按类别避开最近三句；先格式化再比较，报时也不会漏掉去重。 */
export function pickSpeech(
  table: SpeechTable,
  kind: string,
  last: string | null,
  history: SpeechHistory = new Map(),
  random: () => number = Math.random
): string | null {
  const fallback: Record<string, string> = { gentle_drag: "drag", throw: "drag", land: "drag", comfort: "pat" };
  const list = table[kind] ?? table[fallback[kind]] ?? table.idle;
  if (!list?.length) return null;
  const choices = [...new Set(list.map(formatSpeech))];
  const recent = history.get(kind) ?? [];
  let available = choices.filter((text) => text !== last && !recent.includes(text));
  if (!available.length) available = choices.filter((text) => text !== last);
  if (!available.length) available = choices;
  const text = available[Math.floor(random() * available.length)];
  const keep = Math.min(3, choices.length - 1);
  history.set(kind, keep > 0 ? [...recent, text].slice(-keep) : []);
  return text;
}
