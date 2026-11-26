import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { chromium } from '../../../e2e/node_modules/playwright-core/index.mjs';

const browser = await chromium.launch({ channel: 'msedge' });
const shot = name => fileURLToPath(new URL('shots/' + name + '.png', import.meta.url));
const url = new URL('index.html', import.meta.url).href;
const page = await browser.newPage({ viewport: { width: 1320, height: 1000 } });
const errors = [];
page.on('pageerror', e => errors.push(e.message));
async function open() {
  await page.goto(url);
  await page.evaluate(() => document.fonts.ready);
  await page.addStyleTag({ content: 'html{scroll-behavior:auto}' });
  assert.deepEqual(errors, []);
}
async function still(locator, name) {
  const viewport=page.viewportSize(), height=Math.ceil((await locator.boundingBox()).height)+140;
  if(height>viewport.height)await page.setViewportSize({width:viewport.width,height});
  await locator.evaluate(el=>window.scrollTo({top:el.getBoundingClientRect().top+scrollY-75,behavior:'instant'}));
  await page.waitForTimeout(570);
  await locator.screenshot({ path: shot(name), style: '.top{visibility:hidden}' });
  if(height>viewport.height)await page.setViewportSize(viewport);
}
try {
  await open();
  assert.equal(await page.locator('.live').count(), 8);
  assert.equal(await page.locator('.art .phone').count(), 8);
  const dimensions = await page.locator('.live .phone').evaluateAll(phones => phones.map(p => [p.offsetWidth, p.offsetHeight]));
  dimensions.forEach(d => assert.deepEqual(d, [390, 844]));
  const ids = await page.locator('[id]').evaluateAll(elements => elements.map(e => e.id));
  assert.equal(ids.length, new Set(ids).size);
  await page.screenshot({ path: shot('cover'), clip: { x: 0, y: 0, width: 1320, height: await page.locator('#ideas').evaluate(e => Math.ceil(e.getBoundingClientRect().bottom)) } });
  for (const id of ['home', 'permission', 'account', 'progress']) {
    const art = page.locator('#full-' + id);
    await art.locator('summary').click();
    assert.equal(await art.evaluate(e => e.open), true);
    for (const pane of await art.locator('.pane').all()) {
      assert.equal(await pane.evaluate(e => e.scrollHeight <= e.clientHeight + 1), true);
    }
    await still(art.locator('.twins'), id + '-full');
    await art.locator('summary').click();
  }
  await still(page.locator('#system'), 'system');

  // Preserve the four-screen overview for each theme without changing the prototype.
  for (const theme of ['dark','light']) {
    await page.setViewportSize({ width: 1670, height: 960 });
    await page.evaluate(t => {
      const row = document.createElement('div');
      row.id = 'contact'; row.style.cssText = 'width:1640px;background:var(--stage);display:flex;gap:20px;padding:10px';
      for (const sample of document.querySelectorAll('.sample.live[data-theme="'+t+'"]')) {
        const copy = sample.cloneNode(true); copy.classList.remove('live'); copy.style.cssText = 'width:390px;flex:none;margin:0';
        copy.querySelector('figcaption').append(' · '+sample.querySelector('.phone').getAttribute('aria-label'));
        copy.querySelector('.phone').classList.add('run'); row.append(copy);
      }
      document.body.append(row);
    }, theme);
    await still(page.locator('#contact'), theme);
    await page.locator('#contact').evaluate(e=>e.remove());
  }
  const luminance = rgb => {
    const channels = rgb.match(/[0-9.]+/g).slice(0,3).map(v => Number(v)/255).map(v=>v<=.04045?v/12.92:((v+.055)/1.055)**2.4);
    return channels[0]*.2126+channels[1]*.7152+channels[2]*.0722;
  };
  for (const sel of ['#home-light .capacity','#progress-light .recap']) {
    const colors = await page.locator(sel).evaluate(card => [...card.querySelectorAll('*')].filter(el=>el instanceof HTMLElement && [...el.childNodes].some(n=>n.nodeType===3 && n.textContent.trim())).map(el=>[getComputedStyle(el).color,getComputedStyle(card).backgroundColor]));
    assert.ok(colors.length);
    const minimum=Math.min(...colors.map(([fg,bg])=>{const [a,b]=[luminance(fg),luminance(bg)].sort((a,b)=>a-b);return (b+.05)/(a+.05)}));
    assert.ok(minimum>=4.5);console.log(sel,minimum.toFixed(2)+':1');
  }
  for (const theme of ['dark','light']) {
    const side = '.live[data-theme="'+theme+'"] ';
    const receipt = page.locator('#receipt-'+theme);
    await page.locator('#approve-'+theme).click();
    await page.locator('#cancel-'+theme).click();
    await page.locator('#approve-'+theme).click();
    await page.locator('#confirm-'+theme).click();
    assert.match(await receipt.textContent(), /Approved once/);
    await page.locator(side+'[data-harvest]').click();
    await page.waitForFunction(t => document.querySelector('#account-'+t+' [data-harvest-status]').textContent.includes('40 XP'),theme);
    assert.deepEqual(await page.locator(side+'[data-xp]').allTextContents(), ['1,280','1,280','1,280']);
    assert.equal(await page.locator(side+'[data-harvest]').isDisabled(),true);
    assert.equal(await page.locator(side+'[data-lap]').textContent(),'04');
    assert.match(await page.locator('#rule-'+theme).textContent(),/3 of 4 resets/);
    if(theme==='dark')await still(page.locator('#account-dark .harvest'),'harvest');
    await page.locator(side+'[data-badge="0"]').click();
    assert.match(await page.locator('#rule-'+theme).textContent(),/1 of 1 harvest/);
    await page.locator('#edit-'+theme).click();
    await page.locator(side+'[data-step="1"]').first().click();
    assert.equal(await page.locator(side+'.widget').first().getAttribute('data-widget'),'recommendation');
    await page.locator('#edit-'+theme).click();
    await page.locator('#remind-'+theme).click();
    assert.equal(await page.locator('#remind-'+theme).isDisabled(),true);
    const download=page.waitForEvent('download');await page.locator('#share-'+theme).click();assert.match((await download).suggestedFilename(),/\.png$/);
  }
  await page.emulateMedia({reducedMotion:'reduce'});
  await page.locator('#replay').click();
  assert.equal(await page.locator('#strip [data-xp]').textContent(),'1,280');
  await page.locator('.seg button').last().click();assert.equal(await page.locator('.seg button').last().getAttribute('aria-pressed'),'true');
  await page.locator('[role=switch]').click();assert.equal(await page.locator('[role=switch]').getAttribute('aria-checked'),'false');
  await page.clock.install();await open();
  await page.locator('#approve-light').click();await page.clock.fastForward(253000);
  assert.equal(await page.locator('#auth-light').isVisible(),false);assert.match(await page.locator('#receipt-light').textContent(),/closed/);
  for(const width of [320,390,768,1320]){
    await page.setViewportSize({width,height:900});
    for(const scale of [1,1.5,2]){
      const overflow=await page.evaluate(n=>{
        document.documentElement.style.fontSize=n*100+'%';
        return [document.documentElement,...document.querySelectorAll('.live .pane,.live .phone,.live .decision')].filter(e=>e.scrollWidth>e.clientWidth+1).map(e=>[e.closest('.sample')?.id||'page',e.className,e.scrollWidth,e.clientWidth]);
      },scale);
      assert.deepEqual(overflow,[], 'Horizontal overflow at '+width+'px / '+scale+'x '+JSON.stringify(overflow));
    }
  }
  assert.deepEqual(errors, []);
  console.log('Four paired screens, static artboards, harvest, decision controls, badge details, recap, responsive layout and motion checks passed.');
} finally { await browser.close(); }
