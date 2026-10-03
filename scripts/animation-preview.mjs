import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const sheet = readFileSync(fileURLToPath(new URL('../public/spritesheet-extended.webp', import.meta.url))).toString('base64');
const version=JSON.parse(readFileSync(fileURLToPath(new URL('../package.json',import.meta.url)),'utf8')).version;
const actions = [
  { name:"叼飞盘向右",hint:"咬住蓝色飞盘边缘跑回来",row:21,durations:[120,120,120,120,120,120,120,220] },
  { name:"叼飞盘向左",hint:"空中接盘后松口归还，可再次投掷",row:22,durations:[120,120,120,120,120,120,120,220] },
  { name: "吃饼干", hint: "接住、咬下、嚼动，再舔嘴回应", row: 20, durations: [450,350,350,300,300,300,350,400] },
  { name: "松口向右", hint: "球落在脚边，可以继续抓起抛出", row: 18, durations: [100,100,100,100] },
  { name: "松口向左", hint: "归还的球不会自动收起", row: 19, durations: [100,100,100,100] },
  { name: '咬球向右', hint: '球画进动作帧，上下颚夹住球', row: 16, durations: [120,120,120,120,120,120,120,220] },
  { name: '咬球向左', hint: '保持黑白毛色与红色胸背，咬球跑步', row: 17, durations: [120,120,120,120,120,120,120,220] },
  { name: '摸头开心', hint: '光标轻停约 1.2 秒；生气时也可安抚', row: 11, durations: [180,180,180,180,180,180,180,240] },
  { name: '睡眠呼吸', hint: '三分钟没有互动，安静趴下休息', row: 12, durations: [450,450,450,450,450,450,450,450] },
  { name: '睡醒伸懒腰', hint: '光标碰到睡着的大熊时醒来', row: 13, durations: [200,180,200,240,240,180,180,260] },
  { name: '甩毛恢复', hint: '快速甩动后，等停稳再整理毛毛', row: 14, durations: [180,110,110,110,110,110,140,260] },
  { name: '歪头撒娇', hint: '点击时可能回应；空闲时偶尔自己撒娇', row: 15, durations: [180,200,220,220,300,220,180,260] },
  { name: '思考小动作', hint: '原有动作已加入低频自主行为', row: 7, durations: [120,120,120,120,120,220] },
];
const html = `<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>大熊 ${version} 动作预览</title><style>
body{margin:0;background:#f5efe6;color:#42352c;font:16px system-ui,sans-serif}main{max-width:1000px;margin:40px auto;padding:20px}h1{font-size:30px}p{line-height:1.6}button,select{font:inherit;padding:8px 14px;border:1px solid #bca991;border-radius:8px;background:#fffaf1;color:inherit}.controls{display:flex;gap:12px;flex-wrap:wrap;margin:24px 0}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:18px}.card{padding:20px;border:1px solid #d7c8b8;border-radius:16px;background:#fffaf3}.sprite{width:144px;height:156px;overflow:hidden;position:relative;margin:auto}.sprite img{position:absolute;width:1152px;height:3588px;max-width:none;pointer-events:none}.hint{min-height:48px;font-size:14px}.frame{font-size:12px;color:#766552}body.dark{background:#202631;color:#f3eee7}body.dark .card{background:#333c49;border-color:#586472}body.dark .frame{color:#cec6bc}h2{font-size:18px}
</style><main><h1>大熊的新动作</h1><p>这里循环预览咬盘、投喂、咬球、松口和日常小动作的所有帧；桌面宠物会按实际互动场景触发。</p><div class="controls"><button id="pause">暂停</button><label>速度 <select id="speed"><option value="0.5">0.5×</option><option value="1" selected>1×</option><option value="2">2×</option></select></label><button id="theme">切换深色背景</button></div><div class="grid">${actions.map(a=>`<section class="card"><h2>${a.name}</h2><div class="sprite"><img alt="${a.name}"  /></div><p class="hint">${a.hint}</p><span class="frame"></span></section>`).join('')}</div></main><script>
const sheet="data:image/webp;base64,${sheet}";const actions=${JSON.stringify(actions)};const cards=[...document.querySelectorAll('.card')];let elapsed=0,previous=performance.now(),paused=false;cards.forEach(card=>card.querySelector('img').src=sheet);
document.querySelector('#pause').onclick=(e)=>{paused=!paused;e.target.textContent=paused?'继续播放':'暂停'};document.querySelector('#theme').onclick=()=>document.body.classList.toggle('dark');
function render(now){const dt=Math.min(now-previous,100);previous=now;if(!paused)elapsed+=dt*Number(document.querySelector('#speed').value);actions.forEach((a,i)=>{const duration=a.durations.reduce((x,y)=>x+y,0);let t=elapsed%duration,col=0;while(col<a.durations.length-1&&t>=a.durations[col]){t-=a.durations[col];col++}cards[i].querySelector('img').style.transform='translate('+(-col*144)+'px,'+(-a.row*156)+'px)';cards[i].querySelector('.frame').textContent='第 '+(col+1)+' / '+a.durations.length+' 帧'});requestAnimationFrame(render)}requestAnimationFrame(render);
</script></html>`;
writeFileSync(process.argv[2] || 'animation-preview.html', html);
