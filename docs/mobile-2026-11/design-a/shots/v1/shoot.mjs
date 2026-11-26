// Renders the review page: live phones, full-scroll artboards, moments, and the 1080 x 1350 recap images.
import { chromium } from 'file:///D:/Projects/abushanab-net/HarnessManager-rc-chat/e2e/node_modules/playwright-core/index.mjs';
const url = 'file:///D:/Projects/abushanab-net/HarnessManager-rc-chat/docs/mobile-2026-11/design-a/index.html';
const out = 'D:/Projects/abushanab-net/HarnessManager-rc-chat/docs/mobile-2026-11/design-a/shots/';
const b = await chromium.launch({ channel: 'msedge' });
const p = await b.newPage({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: 1 });
const errs = []; p.on('console', m => { if (m.type() === 'error') errs.push(m.text()); }); p.on('pageerror', e => errs.push(e.message));
await p.goto(url); await p.waitForTimeout(3200);
await p.screenshot({ path: out + 'page-top.png' });
await p.addStyleTag({ content: '.topbar{display:none!important}' });  // keep the sticky bar out of element captures
await p.locator('#system').screenshot({ path: out + 'system.png' });
const name = async el => (await el.getAttribute('data-screen')) + '-' + (await el.getAttribute('data-theme'));
const live = p.locator('.screen-set .phone:not([data-static])');
for (let i = 0; i < await live.count(); i++) await live.nth(i).screenshot({ path: out + await name(live.nth(i)) + '.png' });
const boards = p.locator('.phone[data-static]');
for (let i = 0; i < await boards.count(); i++) await boards.nth(i).screenshot({ path: out + 'full-' + await name(boards.nth(i)) + '.png' });
for (const id of ['m-harvest', 'm-level', 'm-badge1', 'm-badge2']) await p.locator('#' + id).screenshot({ path: out + 'moment-' + id.slice(2) + '.png' });
await p.locator('#s7').screenshot({ path: out + 'set-home.png' });
// recap at full size: drop the 40% preview scale for the capture
await p.addStyleTag({ content: '.recap-frame{width:auto!important;height:auto!important;overflow:visible!important}.recap-img{transform:none!important}' });
await p.waitForTimeout(200);
await p.locator('#recap-light').screenshot({ path: out + 'recap-1080x1350-light.png' });
await p.locator('#recap-dark').screenshot({ path: out + 'recap-1080x1350-dark.png' });
console.log('errors:', JSON.stringify(errs));
await b.close();
