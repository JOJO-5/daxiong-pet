#!/usr/bin/env node
// Browser rendering regression; requires a running Vite server and Chromium.
// IPC is a synthetic fixture. This does not certify native Windows compositing.
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const out = process.env.FRAME_RENDER_OUT || 'test-results/frame-render';
await mkdir(out, { recursive: true });
const profile = await mkdtemp(join(tmpdir(), 'daxiong-frame-'));
const browser = spawn(process.env.CHROMIUM || '/usr/bin/chromium', [
  '--headless', '--no-sandbox', '--disable-dev-shm-usage', '--remote-debugging-port=0',
  `--user-data-dir=${profile}`, 'about:blank',
], { stdio: 'ignore' });
let socket;
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
try {
  let port;
  for (let i = 0; i < 100; i++) {
    try { port = (await readFile(join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]; break; }
    catch { await delay(100); }
  }
  if (!port) throw Error('Chromium did not start');
  const pages = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
  socket = new WebSocket(pages.find(page => page.type === 'page').webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let id = 0;
  const pending = new Map();
  socket.onmessage = event => {
    const message = JSON.parse(event.data);
    if (!message.id) return;
    const waiter = pending.get(message.id);
    if (!waiter) return;
    pending.delete(message.id);
    message.error ? waiter.reject(Error(JSON.stringify(message.error))) : waiter.resolve(message.result);
  };
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const request = ++id;
    const timeout = setTimeout(() => { pending.delete(request); reject(Error(`CDP timeout: ${method}`)); }, 60000);
    pending.set(request, { resolve: value => { clearTimeout(timeout); resolve(value); }, reject: error => { clearTimeout(timeout); reject(error); } });
    socket.send(JSON.stringify({ id: request, method, params }));
  });
  const evaluate = async expression => {
    const result = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description || result.exceptionDetails.text);
    return result.result.value;
  };
  await send('Page.enable');
  await send('Page.addScriptToEvaluateOnNewDocument', { source: `
    localStorage.setItem('pet-menu-intro-v2','seen');
    const callbacks=new Map(),listeners=new Map();let next=0;
    window.__TAURI_INTERNALS__={
      transformCallback(cb){const id=++next;callbacks.set(id,cb);return id},
      unregisterCallback(id){callbacks.delete(id)},
      invoke(command,args){
        if(command==='plugin:event|listen'){const id=++next;listeners.set(id,args);return Promise.resolve(id)}
        if(command==='plugin:event|unlisten'){listeners.delete(args.eventId);return Promise.resolve()}
        if(command==='current_pet')return Promise.resolve({id:'__builtin__',name:'大熊',rows:27,data_url:null});
        return Promise.resolve(null);
      }
    };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(_,id){listeners.delete(id)}};
    window.__frameEmit=(event,payload)=>{for(const [id,l] of listeners){if(l.event===event)callbacks.get(l.handler)?.({event,id,payload})}};
  ` });
  await send('Page.navigate', { url: process.env.FRAME_RENDER_URL || 'http://127.0.0.1:1420' });
  for (let i = 0; i < 100; i++) {
    if (await evaluate("!!document.querySelector('.pet-sheet')?.complete && !!document.querySelector('canvas.pet-frame')")) break;
    if (i === 99) throw Error('isolated frame canvas missing or image not loaded');
    await delay(100);
  }
  const checks = await evaluate(`(async()=>{
    const checks=[];
    const assert=(ok,message)=>{if(!ok)throw Error(message);checks.push(message)};
    const painted=()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
    const emit=async(row,col,scale=1)=>{window.__frameEmit('pet:frame',{row,col,pose_scale:scale});await painted()};
    const source=document.querySelector('.pet-sheet');await source.decode();
    assert(getComputedStyle(source).display==='none','atlas source never becomes a visible compositor layer');
    const atlas=document.createElement('canvas');atlas.width=1536;atlas.height=source.naturalHeight;
    const context=atlas.getContext('2d',{willReadFrequently:true});context.drawImage(source,0,0);
    const reference=context.getImageData(0,0,atlas.width,atlas.height).data;
    const view=document.querySelector('canvas.pet-frame');
    assert(view.width===192&&view.height===208,'visible backing texture is exactly one original cell');
    const verify=(row,col)=>{
      const actual=view.getContext('2d').getImageData(0,0,192,208).data;
      for(let y=0;y<208;y++)for(let x=0;x<192;x++)for(let c=0;c<4;c++){
        if(actual[(y*192+x)*4+c]!==reference[((row*208+y)*1536+col*192+x)*4+c])
          throw Error('pixel mismatch at row '+row+' col '+col+' x '+x+' y '+y);
      }
    };
    // Includes sparse/unused cells: these must erase all old tail/prop pixels.
    for(let row=0;row<27;row++)for(let col=0;col<8;col++){await emit(row,col);verify(row,col)}
    assert(true,'all 216 atlas cells match original RGBA, including empty cells and frame transitions');
    await emit(16,5);await emit(0,7);
    assert(view.getContext('2d').getImageData(0,0,192,208).data.every(n=>n===0),'sparse next frame erases previous tail and ball');
    await emit(1,8);
    assert(view.getContext('2d').getImageData(0,0,192,208).data.every(n=>n===0),'invalid column cannot sample adjacent cells');
    const legacy=document.createElement('canvas');legacy.width=1536;legacy.height=9*208;
    legacy.getContext('2d').drawImage(source,0,0);
    window.__frameEmit('pet:switch',{id:'legacy-fixture',name:'legacy',rows:9,data_url:legacy.toDataURL()});
    for(let i=0;i<100&&document.querySelector('.pet-sheet').naturalHeight!==9*208;i++)await painted();
    await document.querySelector('.pet-sheet').decode();await emit(8,5);verify(8,5);
    assert(document.querySelector('.pet-sheet').naturalHeight===1872,'nine-row legacy pet decodes and renders without stretching');
    window.__frameEmit('pet:switch',{id:'__builtin__',name:'大熊',rows:27,data_url:null});
    for(let i=0;i<100&&document.querySelector('.pet-sheet').naturalHeight!==27*208;i++)await painted();
    await document.querySelector('.pet-sheet').decode();await emit(25,3,1.5);verify(25,3);
    assert(document.querySelector('.pet-sheet').naturalHeight===5616,'switch back restores exact builtin pixels');
    const r=document.querySelector('.pet-clip').getBoundingClientRect();
    assert(Math.abs(r.width-216)<.01&&Math.abs(r.height-234)<.01,'lying scale preserves native viewport and ground transform');
    return checks;
  })()`);
  for (const dpi of [1, 1.25, 2]) {
    await send('Emulation.setDeviceMetricsOverride', { width: 400, height: 300, deviceScaleFactor: dpi, mobile: false });
    await evaluate("window.__frameEmit('pet:frame',{row:17,col:6,pose_scale:1})");
    await delay(100);
    const screenshot = await send('Page.captureScreenshot', { format: 'png' });
    await writeFile(join(out, `dpr-${dpi}.png`), Buffer.from(screenshot.data, 'base64'));
  }
  await writeFile(join(out, 'result.json'), JSON.stringify({ checks, dpr_screenshots: [1, 1.25, 2], scope: 'Chromium real React renderer; synthetic Tauri IPC, not native OS input or Windows GPU certification' }, null, 2));
  console.log(`${checks.length} pixel/render checks passed; DPR 1, 1.25, 2 screenshots saved.`);
} catch (error) {
  await writeFile(join(out, 'result.json'), JSON.stringify({ error: String(error) }, null, 2));
  throw error;
} finally {
  socket?.close();
  browser.kill('SIGTERM');
  await new Promise(resolve => { if(browser.exitCode!==null)resolve();else browser.once('exit',resolve) });
  await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}
