// The popup by the tray icon: every account's limit ring, as the sidebar draws them, in the window's theme; a ring
// opens the window at that account. Redrawn after each background refresh.

import { S, call, applyTheme } from './core.js';
import { drawNavLimits } from './accounts.js';

const draw = () => call('state').then((st) => { S.state = st; applyTheme(st); drawNavLimits(); }).catch(() => {});
draw();
const ev = window.__TAURI__ && window.__TAURI__.event;
if (ev) ev.listen('refreshed', draw);
document.addEventListener('click', (e) => {
  const a = e.target.closest('a.ring');
  if (a) { e.preventDefault(); call('open_main', { hash: a.getAttribute('href') }).catch(() => {}); }
});
