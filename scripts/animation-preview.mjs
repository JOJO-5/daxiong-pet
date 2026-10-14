import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const sheet = readFileSync(fileURLToPath(new URL('../public/spritesheet-extended.webp', import.meta.url))).toString('base64');
const version=JSON.parse(readFileSync(fileURLToPath(new URL('../package.json',import.meta.url)),'utf8')).version;
// Same fixed calibration as the native frame payload (src-tauri/src/pose.rs).
const poseCalibration=readFileSync(fileURLToPath(new URL('../assets/animation-source/pose-scales.csv',import.meta.url)),'utf8')
  .trim().split('\n').slice(1).map(line=>{const [row,first,last,scale]=line.split(',').map(Number);return {row,first,last,scale}});
const poseScale=(row,col)=>poseCalibration.find(p=>p.row===row&&col>=p.first&&col<=p.last)?.scale??1;
// Read native timing so this review tool cannot invent a last-frame pause.
const atlasSource=readFileSync(fileURLToPath(new URL('../src-tauri/src/atlas.rs',import.meta.url)),'utf8');
const timingNames={0:'D_IDLE',3:'D_WAVE',4:'D_JUMP',5:'D_FAIL',6:'D_WAIT',7:'D_RUN6',8:'D_REVIEW',1:'D_LOCOMOTION',2:'D_LOCOMOTION',11:'D_HAPPY',12:'D_SLEEP',13:'D_WAKE',14:'D_SHAKE',15:'D_AFFECTION',16:'D_LOCOMOTION',17:'D_LOCOMOTION',18:'D_DROP',19:'D_DROP',20:'D_EAT',21:'D_LOCOMOTION',22:'D_LOCOMOTION',23:'D_RUN8',24:'D_RUN8',25:'D_HAPPY',26:'D_HAPPY'};
function nativeDurations(row) {
  const name=timingNames[row];
  const match=atlasSource.match(new RegExp('const '+name+': &\\[u16\\] = &\\[([^\\]]+)\\];'));
  if(!match)throw new Error('Missing native duration track for row '+row);
  const body=match[1].trim();
  return body.includes(';') ? Array(Number(body.split(';')[1])).fill(Number(body.split(';')[0])) : body.split(',').map(Number);
}
const actions = [
  {name:"原版站立（体型基准）",hint:"看头部、胸背带与落脚线；躺姿不能按高度对齐",row:0,durations:nativeDurations(0)},
  {name:"摸肚皮侧躺",hint:"1.5×整体校正，保留关节和原始头身比例",row:25,durations:nativeDurations(25)},
  {name:"躺下与起身",hint:"站立/伏低保持1×，横躺四帧使用1.5×，落地线固定",row:26,durations:nativeDurations(26)},
  { name:"原版向右（形象基准）",hint:"对照脸型、毛色、嘴鼻、腿长与红胸背",row:1,durations:nativeDurations(1) },
  { name:"咬绳向右",hint:"原版形象，闭嘴夹绳、撑地用力与松劲",row:23,durations:nativeDurations(23) },
  { name:"原版向左（形象基准）",hint:"原始图集保持原样，不能以新素材代替基准",row:2,durations:nativeDurations(2) },
  { name:"咬绳向左",hint:"检查每帧咬住绳端，和原版比例是否一致",row:24,durations:nativeDurations(24) },
  { name:"叼飞盘向右",hint:"咬住蓝色飞盘边缘跑回来",row:21,durations:nativeDurations(21) },
  { name:"叼飞盘向左",hint:"空中接盘后松口归还，可再次投掷",row:22,durations:nativeDurations(22) },
  { name: "吃饼干", hint: "接住、咬下、嚼动，再舔嘴回应", row: 20, durations:nativeDurations(20) },
  { name: "松口向右", hint: "球落在脚边，可以继续抓起抛出", row: 18, durations:nativeDurations(18) },
  { name: "松口向左", hint: "归还的球不会自动收起", row: 19, durations:nativeDurations(19) },
  { name: '咬球向右', hint: '球画进动作帧，上下颚夹住球', row: 16, durations:nativeDurations(16) },
  { name: '咬球向左', hint: '保持黑白毛色与红色胸背，咬球跑步', row: 17, durations:nativeDurations(17) },
  { name: '摸头开心', hint: '光标轻停约 1.2 秒；生气时也可安抚', row: 11, durations:nativeDurations(11) },
  { name: '睡眠呼吸', hint: '三分钟没有互动，安静趴下休息', row: 12, durations:nativeDurations(12) },
  { name: '睡醒伸懒腰', hint: '光标碰到睡着的大熊时醒来', row: 13, durations:nativeDurations(13) },
  { name: '甩毛恢复', hint: '快速甩动后，等停稳再整理毛毛', row: 14, durations:nativeDurations(14) },
  { name: '歪头撒娇', hint: '点击时可能回应；空闲时偶尔自己撒娇', row: 15, durations:nativeDurations(15) },
  { name: '思考小动作', hint: '原有动作已加入低频自主行为', row: 7, durations:nativeDurations(7) },
];
actions.push(...[
  [3,"原版挥爪",4],[4,"原版跳跃",5],[5,"原版沮丧/趴下",8],
  [6,"原版期待",6],[8,"原版打量",6],[9,"原版注视上半周",8],[10,"原版注视下半周",8],
].map(([row,name,cols])=>({row,name,hint:row===9||row===10?"注视由鼠标角度控制；本页仅逐角度展示":"保留原版尺寸和方向，逐帧与新动作对照",durations:row===9||row===10?Array(cols).fill(180):nativeDurations(row)})));
const html = `<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>大熊 ${version} 动作预览</title><style>
body{margin:0;background:#f5efe6;color:#42352c;font:16px system-ui,sans-serif}main{max-width:1000px;margin:40px auto;padding:20px}h1{font-size:30px}p{line-height:1.6}button,select{font:inherit;padding:8px 14px;border:1px solid #bca991;border-radius:8px;background:#fffaf1;color:inherit}.controls{display:flex;gap:12px;flex-wrap:wrap;margin:24px 0}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:18px}.card{padding:20px;border:1px solid #d7c8b8;border-radius:16px;background:#fffaf3}.sprite{width:220px;height:156px;position:relative;margin:auto;border-bottom:1px dashed #b49c83}.pose{position:absolute;left:38px;top:0;width:144px;height:156px;overflow:hidden;transform-origin:72px 152.25px}.sprite img{position:absolute;width:1152px;height:auto;max-width:none;pointer-events:none}.hint{min-height:48px;font-size:14px}.frame{font-size:12px;color:#766552}body.dark{background:#202631;color:#f3eee7}body.dark .card{background:#333c49;border-color:#586472}body.dark .frame{color:#cec6bc}h2{font-size:18px}
</style><main><h1>大熊的新动作</h1><p>27行动作全部列出，以原版站立、同向跑动和趴下作基准。比较头部、躯干和落脚线，不能要求躺姿与站姿一样高；暂停后可逐帧核对。</p><div class="controls"><button id="pause">暂停</button><label>速度 <select id="speed"><option value="0.5">0.5×</option><option value="1" selected>1×</option><option value="2">2×</option></select></label><button id="theme">切换深色背景</button><label>暂停帧 <input id="frame" type="range" min="0" max="7" value="0" /></label></div><div class="grid">${actions.map(a=>`<section class="card"><h2>${a.name}</h2><div class="sprite"><div class="pose"><img alt="${a.name}"  /></div></div><p class="hint">${a.hint}</p><span class="frame"></span></section>`).join('')}</div></main><script>
const poseCalibration=${JSON.stringify(poseCalibration)};const poseScale=${poseScale.toString()};const sheet="data:image/webp;base64,${sheet}";const actions=${JSON.stringify(actions)};const cards=[...document.querySelectorAll('.card')];let elapsed=0,previous=performance.now(),paused=false;cards.forEach(card=>card.querySelector('img').src=sheet);
document.querySelector('#pause').onclick=(e)=>{paused=!paused;e.target.textContent=paused?'继续播放':'暂停'};document.querySelector('#theme').onclick=()=>document.body.classList.toggle('dark');document.querySelector('#frame').oninput=()=>{paused=true;document.querySelector('#pause').textContent='继续播放'};
function render(now){const dt=Math.min(now-previous,100);previous=now;if(!paused)elapsed+=dt*Number(document.querySelector('#speed').value);actions.forEach((a,i)=>{const duration=a.durations.reduce((x,y)=>x+y,0);let t=elapsed%duration,col=0;while(col<a.durations.length-1&&t>=a.durations[col]){t-=a.durations[col];col++}if(paused)col=Math.min(Number(document.querySelector('#frame').value),a.durations.length-1);cards[i].querySelector('.pose').style.transform='scale('+poseScale(a.row,col)+')';cards[i].querySelector('img').style.transform='translate('+(-col*144)+'px,'+(-a.row*156)+'px)';cards[i].querySelector('.frame').textContent='第 '+(col+1)+' / '+a.durations.length+' 帧 · '+poseScale(a.row,col)+'×'});requestAnimationFrame(render)}requestAnimationFrame(render);
</script></html>`;
writeFileSync(process.argv[2] || 'animation-preview.html', html);
