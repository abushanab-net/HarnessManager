// The popup by the tray icon: every account's limit ring, as the reference's tray draws them, in the window's theme;
// a ring opens the window at that account. Redrawn after each background refresh.

import { S, call, applyTheme, shown, onShown } from './core.js';
import { brandMark } from './lib.js';
import { drawTray } from './accounts.js';

const root = document.getElementById('tray-root');
const draw = () => call('state').then(async (st) => { S.state = st; applyTheme(st); await drawTray(root); const m = document.getElementById('tray-mark'); if (m) m.innerHTML = brandMark(16); }).catch(() => {});
draw();
// refreshes come about once a second while a session is written; redraw at most every five seconds, never two at once
let timer, drawing = false, again = false, lastDraw = 0;
const redrawSoon = () => { clearTimeout(timer); timer = setTimeout(async () => { if (drawing) { again = true; return; } drawing = true; try { await draw(); } finally { drawing = false; lastDraw = Date.now(); if (again) { again = false; redrawSoon(); } } }, Math.max(250, 5000 - (Date.now() - lastDraw))); };
const ev = window.__TAURI__ && window.__TAURI__.event;
// hidden (as it is until the tray icon is clicked), it draws nothing: it remembers that it is stale and draws when shown
let stale = false;
if (ev) ev.listen('refreshed', async () => { if (await shown()) redrawSoon(); else stale = true; });
if (ev) onShown(() => { if (stale) { stale = false; draw(); } });
// a ring opens the main window at that account, instead of this window's own route
document.addEventListener('click', (e) => {
  const a = e.target.closest('[data-act=tray-open]');
  if (a) { e.stopPropagation(); e.preventDefault(); call('open_main', { hash: '#account?name=' + encodeURIComponent(a.dataset.name) }).catch(() => {}); }
}, true);
