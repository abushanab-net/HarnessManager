// node tools/sheet.mjs page.html name [--full]: one contact sheet of every phone on the page, light row over dark row, at 60%
import { chromium } from 'file:///D:/Projects/abushanab-net/HarnessManager-rc-chat/e2e/node_modules/playwright-core/index.mjs';
const [page, name, flag] = process.argv.slice(2);
const base = 'file:///D:/Projects/abushanab-net/HarnessManager-rc-chat/docs/mobile-2026-11/design-a/';
const b = await chromium.launch({ channel: 'msedge' });
const p = await b.newPage({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: +(process.env.SCALE || .6) });
const errs = []; p.on('console', m => { if (m.type() === 'error') errs.push(m.text()); }); p.on('pageerror', e => errs.push(e.message));
await p.goto(base + page + (flag === '--full' ? '?full' : '')); await p.waitForTimeout(+(process.env.WAIT || 2500));
const only = process.env.ONLY;  // comma list of data-pair keys
await p.evaluate(only => {
  const sets = [...document.querySelectorAll('.screen-set')].filter(s => !only || only.split(',').includes(s.dataset.pair));
  const box = document.createElement('div'); box.id = 'sheet';
  box.style.cssText = 'position:absolute;left:0;top:0;z-index:999;background:#888;padding:12px;display:grid;gap:12px;grid-auto-flow:column;grid-template-rows:auto auto;width:max-content';
  sets.forEach(s => s.querySelectorAll('.device').forEach(d => box.appendChild(d)));
  document.body.prepend(box); document.querySelector('main')?.remove(); document.querySelector('.topbar')?.remove();
}, only);
await p.waitForTimeout(400);
await p.locator('#sheet').screenshot({ path: `D:/Projects/abushanab-net/HarnessManager-rc-chat/docs/mobile-2026-11/design-a/shots/sheet-${name}.png` });
console.log('errors:', JSON.stringify(errs));
await b.close();
