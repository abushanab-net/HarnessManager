// walks the five prototype journeys by clicking, and screenshots each step -> shots/proto-*.png
import { chromium } from 'file:///D:/Projects/abushanab-net/HarnessManager-rc-chat/e2e/node_modules/playwright-core/index.mjs';
const b = await chromium.launch({ channel: 'msedge' }); const p = await b.newPage({ viewport: { width: 1400, height: 1000 }, deviceScaleFactor: .6 });
const errs = []; p.on('pageerror', e => errs.push(e.message)); p.on('console', m => m.type() === 'error' && errs.push(m.text()));
await p.goto('file:///D:/Projects/abushanab-net/HarnessManager-rc-chat/docs/mobile-2026-11/design-a/prototype.html'); await p.waitForTimeout(1500);
const ph = p.locator('.phone[data-proto]'), shot = async n => { await p.waitForTimeout(700); await ph.screenshot({ path: `shots/proto-${n}.png` }); };
const tap = async sel => { await ph.locator(sel).first().click(); };
const J = async i => { await p.locator(`.jr-go[data-j="${i}"]`).click(); await p.waitForTimeout(400); };
await J(0); await tap('[data-to="scan"]'); await tap('.vf'); await shot('1-approve'); await p.waitForTimeout(3300); await shot('1-connected'); await tap('[data-to="notify"]'); await tap('[data-to="home"]'); await shot('1-home');
await J(1); await tap('.req-row[data-to="permission"]'); await shot('2-permission'); await tap('[data-approve]'); await p.waitForTimeout(900); await shot('2-receipt'); await tap('.receipt [data-to="home"]'); await shot('2-home');
await J(2); await tap('.un'); await shot('3-account');
await J(3); await tap('.ss-row'); await shot('4-session'); await tap('.stop-btn'); await shot('4-stop'); await tap('.stop-sheet [data-to="stopped"]'); await shot('4-stopped');
await J(4); await tap('.you-lvl'); await tap('[data-badge="sweep"]'); await shot('5-badge');
const steps = await p.$$eval('.jr li', l => l.map(x => (x.classList.contains('done') ? 'x' : x.classList.contains('now') ? '>' : '.')).join(''));
console.log('step marks', steps, 'errors', JSON.stringify(errs)); await b.close();
