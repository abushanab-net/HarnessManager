// The popup by the tray icon: every account's limit ring, as the reference's tray draws them, in the window's theme;
// a ring opens the window at that account. Redrawn after each background refresh.

import { S, call, applyTheme } from './core.js';
import { brandMark } from './lib.js';
import { drawTray } from './accounts.js';

const root = document.getElementById('tray-root');
const draw = () => call('state').then(async (st) => { S.state = st; applyTheme(st); await drawTray(root); const m = document.getElementById('tray-mark'); if (m) m.innerHTML = brandMark(16); }).catch(() => {});
draw();
const ev = window.__TAURI__ && window.__TAURI__.event;
if (ev) ev.listen('refreshed', draw);
// a ring opens the main window at that account, instead of this window's own route
document.addEventListener('click', (e) => {
  const a = e.target.closest('[data-act=tray-open]');
  if (a) { e.stopPropagation(); e.preventDefault(); call('open_main', { hash: '#account?name=' + encodeURIComponent(a.dataset.name) }).catch(() => {}); }
}, true);
