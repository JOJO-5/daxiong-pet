/** Conservative text cues, not a model emotion diagnosis. Serious topics stay calm. */
export function replyMotion(text: string): "calm" | "curious" | "happy" | "gentle" {
  if (/难过|伤心|焦虑|害怕|生病|去世|失去|疼|疲惫|累了|压力|对不起|抱歉|没答出来|失败|先不说/.test(text)) return "calm";
  if (/[?？]/.test(text)) return "curious";
  if (/太好了|好开心|真开心|恭喜|成功啦|做到了/.test(text)) return "happy";
  return "gentle";
}
