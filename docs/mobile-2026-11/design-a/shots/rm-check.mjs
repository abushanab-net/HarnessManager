// Reduce Motion check: moments must land in their end state with no movement.
import { chromium } from 'file:///D:/Projects/abushanab-net/HarnessManager-rc-chat/e2e/node_modules/playwright-core/index.mjs';
const out = 'D:/Projects/abushanab-net/HarnessManager-rc-chat/docs/mobile-2026-11/design-a/shots/';
const b = await chromium.launch({ channel: 'msedge' });
const p = await b.newPage({ viewport: { width: 1600, height: 1000 }, reducedMotion: 'reduce' });
const errs = []; p.on('pageerror', e => errs.push(e.message));
await p.goto('file:///D:/Projects/abushanab-net/HarnessManager-rc-chat/docs/mobile-2026-11/design-a/you.html');
await p.addStyleTag({ content: '.topbar{display:none!important}' });
await p.waitForTimeout(400);
await p.locator('[data-play=harvest][data-for="#m-harvest"]').click(); await p.waitForTimeout(80);
await p.locator('#m-harvest').screenshot({ path: out + 'rm-harvest.png' });
console.log('xp now', await p.locator('#m-harvest [data-xp]').textContent());
await p.locator('[data-play=levelup][data-for="#m-level"]').click(); await p.waitForTimeout(80);
await p.locator('#m-level').screenshot({ path: out + 'rm-levelup.png' });
await p.locator('#s18 .phone[data-theme=light]:not([data-static]) [data-badge=fortnight]').click(); await p.waitForTimeout(80);
await p.locator('#s18 .phone[data-theme=light]:not([data-static])').screenshot({ path: out + 'rm-badge-fortnight.png' });
console.log('errors', JSON.stringify(errs));
await b.close();
