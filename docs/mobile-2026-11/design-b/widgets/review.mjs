import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';
import {chromium} from '../../../../e2e/node_modules/playwright-core/index.mjs';

const url=new URL('widgets.html',import.meta.url).href;
const shot=name=>fileURLToPath(new URL('shots/'+name+'.png',import.meta.url));
const browser=await chromium.launch({channel:'msedge'});
try {
 const page=await browser.newPage({viewport:{width:1420,height:1050},offline:true});
 const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(url);await page.evaluate(()=>document.fonts.ready);
 assert.deepEqual(await page.evaluate(()=>['News','Hanken','Jet'].map(font=>[...document.fonts].some(f=>f.family===font&&f.status==='loaded'))),[true,true,true]);
 await page.screenshot({path:shot('collection')});
 const sets=[
  ['home',['small','medium','large']],
  ['accessories',['circle','rect','inline']],
  ['activities',['activity','compact','minimal','expanded']],
  ['android',['android:square','android:wide','android:tall','notification']]
 ];
 let inspected=0;
 const issues=[];
 function textClipping(specs){
  const problems=[];
  for(const spec of specs){
   const walker=document.createTreeWalker(spec,NodeFilter.SHOW_TEXT);
   const range=document.createRange();
   for(let node=walker.nextNode();node;node=walker.nextNode()){
    const parent=node.parentElement;
    if(!(parent instanceof HTMLElement)||!parent.checkVisibility())continue;
    const frame=spec.getBoundingClientRect();
    for(const match of node.data.matchAll(/[\p{L}\p{N}%+]+/gu)){
     range.setStart(node,match.index);range.setEnd(node,match.index+match[0].length);
     const runs=[...range.getClientRects()].filter(r=>r.width&&r.height);
     if(runs.length>1)problems.push('Split word '+match[0]);
     for(const r of runs){
      if(r.left<frame.left-1||r.right>frame.right+1||r.top<frame.top-1||r.bottom>frame.bottom+1)problems.push('Clipped '+match[0]);
      if(spec.classList.contains('circ')){
       const dx=Math.max(Math.abs(r.left-frame.x-frame.width/2),Math.abs(r.right-frame.x-frame.width/2));
       const dy=Math.max(Math.abs(r.top-frame.y-frame.height/2),Math.abs(r.bottom-frame.y-frame.height/2));
       if(Math.hypot(dx,dy)>23)problems.push('Touches gauge '+match[0]);
      }
     }
    }
   }
  }
  return problems;
 }
 for(const [name,entries] of sets){
  await page.setViewportSize({width:1700,height:1100});
  await page.evaluate(()=>{const plate=document.createElement('section');plate.id='proof';plate.style.cssText='width:1680px;margin:20px auto;padding:24px;background:#f7f5ee';document.body.append(plate)});
  for(const entry of entries){
   const [id,size]=entry.split(':');
   await page.locator('[data-id="'+id+'"]').click();
   if(size)await page.locator('[data-size="'+size+'"]').click();
   assert.equal(await page.locator('#examples .spec').count(),4);
   for(const state of ['stale','fresh']){
    await page.locator('#'+state).click();
    const body=await page.locator('#examples').innerText();
    assert.doesNotMatch(body,/request record|attention record|contact record|session contact|contact seal|\bclosed\b|5h left|read 09:33/i);
    if(['small','large','android'].includes(id)){
     const pick=await page.locator('#examples .variant').first().innerText();
     assert.match(pick,/claude-Resal-Dev01/);assert.match(pick,/64% erased Sat/);
    }
    assert.match(body,state==='stale'?/Stale|Lost contact/:/8m ago|just now/);
    if(id==='compact')assert.equal((await page.locator('#examples .variant').last().locator('.compact span:last-child').first().textContent()),state==='fresh'?'1':'—');
    if(id==='activity')assert.match(body,/If not answered, it waits in the terminal/);
    if(id==='android'&&size==='square')assert.doesNotMatch(await page.locator('#examples .earned .andMetric').first().innerText(),/Earned/);
    if(id==='circle'){
     const gauge=page.locator('#examples .circ').first();
     assert.match(await gauge.getAttribute('aria-label'),/5-hour room, 93%/);
     assert.doesNotMatch(await gauge.innerText(),/5-hour room/);
    }
    const clipped=await page.locator('#examples .spec').evaluateAll(textClipping);
    if(clipped.length)issues.push([id,size,state,...clipped]);
    const over=await page.locator('#examples .spec').evaluateAll(specs=>specs.flatMap(el=>[...el.querySelectorAll('*')].filter(n=>{
     if(!(n instanceof HTMLElement)||!n.checkVisibility())return false;
     const a=el.getBoundingClientRect(),b=n.getBoundingClientRect();
     return b.left<a.left-1||b.right>a.right+1||b.top<a.top-1||b.bottom>a.bottom+1;
    }).map(n=>n.className||n.tagName)));
    if(over.length)issues.push([id,size,state,...over]);
    if(['circle','rect','inline','activity','compact','minimal','expanded','notification'].includes(id)){
     assert.doesNotMatch(await page.locator('#examples .spec').allTextContents().then(t=>t.join(' ')),/sabushanab|Bunyan|DEVELOPMENTPC|npm/);
    }
   }
   if(id==='notification'){
    await page.locator('#examples .noticeShell').evaluateAll(nodes=>nodes.forEach(n=>n.open=true));
    assert.deepEqual(await page.locator('#examples .spec').evaluateAll(textClipping),[]);
    await page.locator('#examples [data-notice=open]').first().click();
    assert.match(await page.locator('#examples .noticeFeedback').first().textContent(),/Unlock/);
    await page.locator('#examples .noticeFeedback').first().evaluate(n=>n.textContent='');
    await page.locator('#examples').screenshot({path:shot('notification-expanded')});
    await page.locator('#examples [data-notice=stop]').first().click();
    assert.match(await page.locator('#examples').innerText(),/Stopped following/);
    await page.locator('#fresh').click();
   }
   if(id==='android'&&size==='tall'){
    await page.locator('#palette').click();assert.equal(await page.locator('#examples .dynamic').count(),0);
    await page.locator('#examples').screenshot({path:shot('android-calibre')});
    await page.locator('#palette').click();assert.equal(await page.locator('#examples .dynamic').count(),4);
   }
   await page.evaluate(({size})=>{
    const row=document.createElement('section');row.style.cssText='margin-bottom:15px;border-top:1px solid #aeb4a6;padding-top:9px';
    const heading=document.createElement('h2');heading.style.cssText='font:400 26px News,serif;margin:0';heading.textContent=document.querySelector('#familyTitle').textContent+(size?' · '+document.querySelector('#sizes [aria-pressed=true]').textContent:'');
    const group=document.querySelector('#examples').cloneNode(true);group.removeAttribute('id');group.style.cssText='display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:15px';
    row.append(heading,group);document.querySelector('#proof').append(row);
   },{size});
   inspected+=4;
  }
  const proof=page.locator('#proof');
  const height=Math.ceil((await proof.boundingBox()).height)+100;
  await page.setViewportSize({width:1700,height:Math.max(height,1100)});
  await proof.screenshot({path:shot(name)});await proof.evaluate(e=>e.remove());
 }
 assert.equal(inspected,56);
 await page.locator('[data-id="large"]').click();await page.locator('#stale').click();
 await page.locator('#examples').screenshot({path:shot('offline')});
 assert.match(await page.locator('#examples').innerText(),/Stale/);
 await page.setViewportSize({width:390,height:844});
 for(const width of [320,390,790,980]){
  await page.setViewportSize({width,height:844});
  for(const id of ['small','medium','large','expanded','android','notification']){
   if(width>900)await page.locator('[data-id="'+id+'"]').click();
   else await page.locator('#familySelect').selectOption(id);
   assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),true,'Page overflow '+width+' '+id);
   const spills=await page.locator('#examples .spec').evaluateAll(nodes=>nodes.filter(n=>n.scrollHeight>n.clientHeight+2||n.scrollWidth>n.clientWidth+2).map(n=>n.className));
   if(spills.length)issues.push([width,id,...spills]);
  }
 }
 assert.deepEqual(issues,[]);assert.deepEqual(errors,[]);
 console.log(inspected+' fresh renderings, corresponding offline fixtures, responsive layouts and privacy checks passed.');
}finally{await browser.close()}
