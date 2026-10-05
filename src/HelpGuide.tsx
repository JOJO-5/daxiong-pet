export default function HelpGuide({onClose}:{onClose:()=>void}) {
  return <section className="play-card help-guide" id="play-help" aria-label="玩法与操作" data-testid="help-guide">
    <h2>怎么玩？</h2>
    <p>右键大熊打开菜单。Mac 触控板也可以双指点按；托盘里随时能找到“如何和大熊互动”。</p>
    <ul>
      <li><strong>球和飞盘：</strong>点按钮直接扔，或拖住桌面玩具，移动后松手。叼回来后，抓起脚边的玩具再扔。</li>
      <li><strong>拔河：</strong>按住橙色绳环，向远离大熊的方向轻拉；松手休息。不用拼手速，也没有输赢。</li>
      <li><strong>小指令：</strong>等大熊完成动作，再点“奖励这次指令”。</li>
      <li><strong>找零食：</strong>先放一块，拖到想藏的位置，松手后点“开始寻找”。</li>
    </ul>
    <p className="small">收起面板（Esc）会继续当前游戏；“收起玩具”才会结束。拖动大熊或开始专注也会结束互动。</p>
    <button onClick={onClose}>知道了</button>
  </section>;
}
