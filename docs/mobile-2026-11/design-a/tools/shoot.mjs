// node tools/shoot.mjs page.html [prefix] [--full]  -> shots/<prefix>-<screen>-<v>-<theme>.png, logs console errors
import { chromium } from 'file:///D:/Projects/abushanab-net/HarnessManager-rc-chat/e2e/node_modules/playwright-core/index.mjs';
const [page, prefix = page.replace('.html', ''), flag] = process.argv.slice(2);
const base = 'file:///D:/Projects/abushanab-net/HarnessManager-rc-chat/docs/mobile-2026-11/design-a/';
const out = 'D:/Projects/abushanab-net/HarnessManager-rc-chat/docs/mobile-2026-11/design-a/shots/';
const b = await chromium.launch({ channel: 'msedge' });
const p = await b.newPage({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: 1 });
const errs = []; p.on('console', m => { if (m.type() === 'error') errs.push(m.text()); }); p.on('pageerror', e => errs.push(e.message));
await p.goto(base + page + (flag === '--full' ? '?full' : '')); await p.waitForTimeout(2500);
await p.addStyleTag({ content: '.topbar{display:none!important}' });
const sel = process.env.SEL || '.screen-set .device';
const els = p.locator(sel);
const n = await els.count();
for (let i = 0; i < n; i++) {
  const el = els.nth(i);
  const ph = el.locator('.phone').first();
  const nm = (await ph.count()) ? [await ph.getAttribute('data-screen'), await ph.getAttribute('data-v'), await ph.getAttribute('data-theme')].filter(Boolean).join('-') : 'el' + i;
  await el.screenshot({ path: `${out}${prefix}-${nm}${flag === '--full' ? '-full' : ''}.png` });
}
console.log(n, 'shots; errors:', JSON.stringify(errs));
await b.close();
