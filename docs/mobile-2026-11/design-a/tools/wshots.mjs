// node tools/wshots.mjs : the widget gallery in groups of three families -> shots/wg-N.png
import { chromium } from 'file:///D:/Projects/abushanab-net/HarnessManager-rc-chat/e2e/node_modules/playwright-core/index.mjs';
const b = await chromium.launch({ channel: 'msedge' }); const p = await b.newPage({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: +(process.env.SCALE || .7) });
const errs = []; p.on('pageerror', e => errs.push(e.message)); p.on('console', m => m.type() === 'error' && errs.push(m.text()));
await p.goto('file:///D:/Projects/abushanab-net/HarnessManager-rc-chat/docs/mobile-2026-11/design-a/widgets.html' + (process.env.Q || '')); await p.waitForTimeout(2000);
await p.addStyleTag({ content: '.topbar{display:none!important}' });
const n = await p.locator('.fam').count(), only = process.env.ONLY ? process.env.ONLY.split(',').map(Number) : null;
for (let i = 0; i < n; i += 3) { if (only && !only.includes(i / 3)) continue;
  await p.evaluate(i => document.querySelectorAll('.fam').forEach((f, j) => f.style.display = (j >= i && j < i + 3) ? '' : 'none'), i);
  await p.locator('[data-widgets]').screenshot({ path: 'shots/wg-' + (i / 3) + '.png' }); }
console.log(n, 'families; errors', JSON.stringify(errs)); await b.close();
