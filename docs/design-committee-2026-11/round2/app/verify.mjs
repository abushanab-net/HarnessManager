import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import path from 'node:path';
import fs from 'node:fs/promises';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
const here=path.dirname(fileURLToPath(import.meta.url));
const require=createRequire(path.resolve(here,'../../../../e2e/package.json'));
const {chromium}=require('playwright-core');
const base=pathToFileURL(path.join(here,'index.html')).href;
const edge='C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe';
const shots=path.join(here,'screenshots'), tmp=path.join(here,'tmp');
await fs.mkdir(shots,{recursive:true});await fs.mkdir(tmp,{recursive:true});
const errors=[], checks=[], captures=[];let serial=0;
function check(name,value){assert.ok(value,name);checks.push(name);}
async function browser(width,height,task){
 const profile=path.join(tmp,`edge-${process.pid}-${serial++}`);
 const context=await chromium.launchPersistentContext(profile,{executablePath:edge,headless:true,viewport:{width,height:height+100},downloadsPath:path.join(profile,'downloads'),timeout:60000,args:['--disable-gpu','--no-first-run']});
 let timer;
 try {
  const page=context.pages()[0];page.setDefaultTimeout(10000);
  page.on('pageerror',e=>errors.push(String(e)));page.on('console',m=>{if(m.type()==='error') errors.push(m.text());});
  await Promise.race([task(page),new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error('Capture/test exceeded 60 seconds')),60000);})]);
 }finally{clearTimeout(timer);await context.close();}
}
const goto=async(page,query='')=>{await page.goto(`${base}?motion=still&${query}`,{timeout:60000});await page.evaluate(()=>document.fonts.ready);};
await browser(1536,864,async(page)=>{
 await goto(page,'w=1536&theme=dark');
 check('capability matrix',await page.evaluate(()=>HM.data.TOOLS.every(t=>t.kinds.subscription.supported && t.kinds.apikey.supported===['claude','codex','pi'].includes(t.id) && t.kinds.routed.supported===['claude','pi'].includes(t.id))));
 check('all fixtures obey capabilities/windows',await page.evaluate(()=>[...HM.data.BASE_ACCOUNTS,...HM.data.EXTRA_4_ACCOUNTS].every(a=>HM.data.TOOLS.find(t=>t.id===a.tool).kinds[a.kind].supported && (!a.windows.length || ['claude','codex'].includes(a.tool) && a.kind==='subscription'))));
 check('all 18 sessions shown, counts agree',await page.locator('.live-mini-list .live-mini-row').count()===18);
 await page.locator('[data-action="set-live-pc"][data-pc="samymsi"]').click();
 check('SAMYMSI count equals four rows',await page.locator('.live-mini-list .live-mini-row').count()===4);
 await page.locator('[data-action="set-live-pc"][data-pc="all"]').click();
 await page.evaluate(()=>{window.oldPulse=document.querySelector('.live-pulse-dot');window.oldRail=document.querySelector('.rail');window.oldSearch=document.getElementById('global-search');document.querySelector('main').scrollTop=260;document.querySelector('.live-mini-list').scrollTop=80;HM.state.selectedAccountId='claude-main';HM.redraw();});
 check('redraw retains shell, animation nodes and scrollers',await page.evaluate(()=>oldRail===document.querySelector('.rail')&&oldPulse===document.querySelector('.live-pulse-dot')&&oldSearch===document.getElementById('global-search')&&document.querySelector('main').scrollTop===260&&document.querySelector('.live-mini-list').scrollTop===80&&HM.state.selectedAccountId==='claude-main'));
 await page.evaluate(()=>{const name=document.querySelector('.lane-acct-name');const range=document.createRange();range.selectNodeContents(name);getSelection().removeAllRanges();getSelection().addRange(range);window.beforeSelectedText=getSelection().toString();HM.redraw();});
 check('text selection survives data redraw',await page.evaluate(()=>getSelection().toString()===beforeSelectedText));
 await page.evaluate(()=>HM.navigate('accounts'));
 await page.locator('[data-action="open-add-account"]').click();
 const input=page.locator('#add-name-input');await input.fill('review-profile');
 await input.evaluate(el=>{el.focus();el.setSelectionRange(2,8);window.oldInput=el;window.oldModal=document.querySelector('.modal-card');oldModal.scrollTop=80;window.beforeModalScroll=oldModal.scrollTop;HM.redraw();});
 check('dialog, focused input, caret selection and scroll survive redraw',await page.evaluate(()=>oldInput===document.activeElement&&oldInput.selectionStart===2&&oldInput.selectionEnd===8&&oldModal===document.querySelector('.modal-card')&&oldModal.scrollTop===beforeModalScroll&&HM.state.modal==='add'));
 await page.keyboard.press('Escape');check('Escape restores opener focus',await page.evaluate(()=>document.activeElement.dataset.action==='open-add-account'));
 await page.locator('#global-search').fill('codex');
 check('search retains input node/focus after each keystroke',await page.evaluate(()=>oldSearch===document.activeElement && HM.state.searchQuery==='codex'));
 await page.locator('#global-search').fill('');
 await page.evaluate(()=>HM.navigate('account',{detailAccountId:'service-account-resal',detailTab:'overview'}));
 for(const tab of ['sessions','projects','settings','overview']) {await page.locator(`[data-action="set-detail-tab"][data-tab="${tab}"]`).first().click();check(`account ${tab} tab`,await page.locator(`.page-tabs [data-tab="${tab}"]`).getAttribute('aria-current')==='page');}
 await page.evaluate(()=>{HM.pages.sessions={title:'Sessions',render:s=>'<div id="extension-content">'+s.accounts.length+' accounts</div>',mount(root,state){window.mountedRoot=root;window.mountCount=(window.mountCount||0)+1;return ()=>window.unmounted=true;}};HM.navigate('sessions');HM.redraw();});
 check('registry render/mount contract and redraw',await page.evaluate(()=>document.getElementById('extension-content').textContent==='8 accounts'&&mountCount===1&&mountedRoot===document.getElementById('page-root')));
 await page.evaluate(()=>{HM.pages.sessions.render=()=>'<section id="extension-content">Changed tag</section><div id="trailing-check">Kept once</div>';HM.redraw();});
 check('keyed tag replacement leaves one sibling',await page.locator('#extension-content').count()===1&&await page.locator('#trailing-check').count()===1);
 await page.evaluate(()=>HM.navigate('skills'));check('unregistered placeholder, mount cleanup',await page.evaluate(()=>window.unmounted&&document.querySelector('main').textContent.includes('Page awaiting')));
 await page.evaluate(()=>{HM.state.preset='firstrun';HM.navigate('accounts');});
 check('first run has no forecast, historical usage or setup counts',await page.evaluate(()=>HM.state.metrics.bestRoomAcct===null&&HM.state.accounts.every(a=>!a.windows.length&&!a.tokens7d&&!a.openSessions&&a.advice.includes('No forecast'))&&HM.state.metrics.updatesCount===0&&HM.state.metrics.mcpCount===0&&HM.state.metrics.skillsCount===0&&!document.querySelector('main').textContent.includes('runs out')));
 await page.evaluate(()=>{HM.state.preset='empty';HM.navigate('accounts');});
 await page.locator('[data-action="open-add-account"]').click();await page.locator('[data-action="set-add-tool"][data-tool="codex"]').click();await page.locator('#add-name-input').fill('');await page.locator('[data-action="submit-add-account"]').click();
 check('empty profile name rejected',await page.evaluate(()=>HM.state.customAccounts.length===0&&HM.state.modal==='add'));
 await page.locator('#add-name-input').fill('first');await page.locator('[data-action="submit-add-account"]').click();
 check('new unread account retains empty dataset',await page.evaluate(()=>HM.state.preset==='empty'&&HM.state.customAccounts.length===1&&!HM.state.customAccounts[0].measured&&HM.state.metrics.bestRoomAcct===null&&HM.state.metrics.repliesTodayCount===0));
 await page.locator('[data-action="open-add-account"]').click();await page.locator('[data-action="submit-add-account"]').click();
 check('duplicate profile is rejected',await page.evaluate(()=>HM.state.customAccounts.length===1&&HM.state.modal==='add'&&HM.state.toastText.includes('already exists')));await page.keyboard.press('Escape');
 await page.evaluate(()=>HM.navigate('overview'));check('unread account never recommended as ready',!(await page.locator('main').textContent()).includes('has the most room'));
 await page.evaluate(()=>HM.navigate('account',{detailAccountId:HM.state.customAccounts[0].id,detailTab:'overview'}));
 await page.locator('[data-action="reauth-account"]').click();
 check('reauth uses selected tool and a preview dialog',await page.locator('[role="dialog"]').getAttribute('aria-label')==='Sign in again' && (await page.locator('[role="dialog"]').textContent()).includes('Codex'));await page.keyboard.press('Escape');
 await page.evaluate(()=>{HM.state.customAccounts.push({...HM.state.customAccounts[0],id:'api-preview',name:'api-preview',kind:'apikey',needsLogin:false});HM.navigate('account',{detailAccountId:'api-preview',detailTab:'settings'});});
 check('API-key account exposes key endpoint and model fields',await page.locator('#account-key').count()===1&&await page.locator('#account-endpoint').count()===1&&await page.locator('#account-model').count()===1);
 await page.locator('#account-key').fill('sample-key-preview');await page.locator('[data-save="key"]').click();
 check('key replacement preview clears credential',await page.locator('#account-key').inputValue()===''&&await page.evaluate(()=>!HM.state.accountDraft['api-preview'].key));
 await page.evaluate(()=>{HM.state.preset='12';HM.navigate('account',{detailAccountId:'pi-openrouter',detailTab:'settings'});});
 check('routed account exposes spend cap',await page.locator('#account-cap').count()===1);
 await page.locator('#account-cap').fill('-1');await page.locator('[data-save="cap"]').click();
 check('negative spend cap rejected',await page.evaluate(()=>HM.state.toastText.includes('positive cap')));
 await page.evaluate(()=>{HM.state.preset='8';HM.state.customAccounts=[];HM.navigate('account',{detailAccountId:'service-account-resal',detailTab:'settings'});});
 await page.locator('#account-label').fill('<img src=x onerror=alert(1)>');await page.locator('[data-save="label"]').click();await page.evaluate(()=>HM.navigate('accounts'));
 check('editable labels escaped at HTML boundary',await page.locator('main img').count()===0 && (await page.locator('main').textContent()).includes('<img src=x'));
 await page.evaluate(()=>{HM.data.BASE_ACCOUNTS.find(a=>a.id==='service-account-resal').name='label, with "quote"';HM.redraw();});
 const downloadEvent=page.waitForEvent('download');await page.locator('[data-action="export-csv"]').click();const download=await downloadEvent;
 const csv=await fs.readFile(await download.path(),'utf8');check('CSV quotes editable labels',csv.includes('"label, with ""quote"""'));
 await page.evaluate(()=>{HM.data.BASE_ACCOUNTS.find(a=>a.id==='service-account-resal').name='service-account-resal';HM.navigate('account',{detailAccountId:'claude-main',detailTab:'settings'});});
 await page.locator('[data-action="confirm-remove"]').click();check('main profile hide-only',await page.locator('#remove-folder').count()===0);await page.keyboard.press('Escape');
 await page.evaluate(()=>{HM.state.motion='live';HM.navigate('overview');});
 check('animation phase survives redraw',await page.evaluate(async()=>{await new Promise(requestAnimationFrame);const dot=document.querySelector('.live-pulse-dot'),animation=dot.getAnimations()[0];if(!animation)return false;animation.currentTime=750;HM.redraw();await new Promise(requestAnimationFrame);return dot===document.querySelector('.live-pulse-dot')&&dot.getAnimations()[0]===animation&&animation.currentTime>=750;}));
 await page.emulateMedia({reducedMotion:'reduce'});check('reduced motion has no running animations',await page.evaluate(()=>document.getAnimations().filter(a=>a.playState==='running').length===0));
 await page.emulateMedia({reducedMotion:'no-preference'});await page.evaluate(()=>{HM.state.motion='still';HM.redraw();});check('Still has no running animations',await page.evaluate(()=>document.getAnimations().filter(a=>a.playState==='running').length===0));
 await page.evaluate(()=>{HM.state.modal='folder';HM.state.startFolderAcct='claude-main';HM.redraw();});
 const folder=page.locator('#start-folder-path');await folder.selectOption({index:1});await page.evaluate(()=>HM.redraw());
 check('recent folder selection survives redraw',await page.evaluate(()=>document.getElementById('start-folder-path').value===HM.state.startFolderPath));await page.keyboard.press('Escape');
 // Width switch must model the same layout independently of the enclosing browser.
 await page.locator('[data-action="set-width"][data-w="1280"]').click();
 check('width switch sets exact width and height',await page.evaluate(()=>{const r=document.querySelector('.app-frame').getBoundingClientRect();return r.width===1280&&r.height===720;}));
});
console.log(`Behavior checks passed: ${checks.length}`);
if(process.argv.includes('--checks-only')) {assert.deepEqual(errors,[]);console.log('Zero console errors');process.exit(0);}
const scenarios=[
 ['overview-lower','page=overview&lower=1'],['overview','page=overview'],['accounts','page=accounts'],['account-detail','page=account'],['add-account','page=accounts&modal=add'],['first-run','preset=firstrun'],['12-accounts','preset=12'],['tray','tray=1'],['tray-12','preset=12&tray=1'],['empty','preset=empty'],['first-run-accounts','preset=firstrun&page=accounts'],['account-sessions','page=account&tab=sessions'],['account-projects','page=account&tab=projects'],['account-settings','page=account&tab=settings']
];
for(const [width,height] of [[1280,720],[1536,864],[1920,1080]])for(const theme of ['dark','light'])for(const [name,query] of scenarios){
 const filename=`${name}-${width}-${theme}.png`;
 await browser(width,height,async(page)=>{
  await goto(page,`w=${width}&theme=${theme}&${query}`);
  if(query.includes('lower=1')) await page.locator('main').evaluate(el=>el.scrollTop=el.scrollHeight);
  const diagnostics=await page.evaluate(()=>{
   const frame=document.querySelector('.app-frame'),main=document.querySelector('main'),tray=document.querySelector('.tray-rings-grid');
   const small=[...frame.querySelectorAll('*')].filter(e=>e.getClientRects().length&&e.childNodes.length&&[...e.childNodes].some(n=>n.nodeType===3&&n.textContent.trim())&&parseFloat(getComputedStyle(e).fontSize)<11).map(e=>e.className);
   return {width:frame.offsetWidth,height:frame.offsetHeight,mainOverflow:main.scrollWidth-main.clientWidth,trayOverflow:tray?tray.scrollWidth-tray.clientWidth:0,small,accountCount:HM.state.accounts.length};
  });
  if(name==='first-run') {assert.equal(await page.locator('.detected-profiles .live-mini-row').count(),4);assert.ok(await page.locator('.detected-profiles b').first().evaluate(el=>el.clientWidth>60));}
  assert.equal(diagnostics.width,width);assert.equal(diagnostics.height,height);assert.equal(diagnostics.mainOverflow,0,filename+' horizontal main overflow');assert.equal(diagnostics.trayOverflow,0,filename+' horizontal tray overflow');assert.equal(diagnostics.small.length,0,filename+' small text');
  await page.locator('.app-frame').screenshot({path:path.join(shots,filename),timeout:60000});captures.push({filename,...diagnostics});
 });
 console.log(`Captured ${filename}`);
}
assert.deepEqual(errors,[],'browser console errors');
await fs.writeFile(path.join(here,'verification.json'),JSON.stringify({engine:'Microsoft Edge headless, file://',capturedAt:new Date().toISOString(),sourceHashes:Object.fromEntries(await Promise.all(['index.html','styles.css','views.js','modals.js','data.js','account.js','pages-a.js','pages-b.js'].map(async name=>[name,createHash('sha256').update(await fs.readFile(path.join(here,name))).digest('hex')]))),checks,consoleErrors:errors,captures},null,2)+'\n');
console.log(`Verified ${checks.length} behavior checks and ${captures.length} captures; zero console errors.`);

