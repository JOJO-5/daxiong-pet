export function fetchControls(phase: string, frisbee: boolean) {
  const canThrow = ["off", "ready", "returned"].includes(phase);
  const waiting = phase === "held" ? "松手就会扔出" : phase === "releasing" ? "正在放到脚边" : "大熊正在捡回来";
  return {
    canShow: phase === "off",
    canThrow,
    throwLabel: canThrow ? phase === "returned" ? "再扔一次" : frisbee ? "扔飞盘" : "抛一球" : waiting,
  };
}

export function interactionActive(playPhase: string, activityPhase: string) {
  return playPhase !== "off" || !["off", "blocked", "cancelled", "found"].includes(activityPhase);
}
