// Starts the window: icons, navigation, search, and live refresh after background imports.

import { S, render, go, closeDialog, call, setAppUpdates, setNavCounts, setFresh, applyTheme, toast, drawPins, restoreFolds } from './core.js';
import { ic, brandMark } from './lib.js';
import './accounts.js';
import { setTodayBadge } from './home.js';
import { setSelfUpdate, refreshProjects } from './views.js';
import './mcp.js';
import './skills.js';
import './apps.js';
import './guide.js';
import './computers.js';

document.documentElement.dataset.theme = 'dark';
document.getElementById('brand-mark').innerHTML = brandMark(28);
document.querySelectorAll('[data-icon]').forEach((el) => { el.outerHTML = ic[el.dataset.icon] || ''; });

// moving to another page closes any dialog left open
window.addEventListener('hashchange', () => { closeDialog(); render(); });

const find = document.getElementById('find');
find.addEventListener('input', () => { if (S.route.view === 'accounts') render(); else if (S.pageFind) S.pageFind(find.value); });
find.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && S.route.view !== 'accounts' && !S.pageFind) { go('sessions', { q: find.value }); find.value = ''; }
});

// the backend imports usage in the background, and the folder watcher refreshes the index, and both say when done
const ev = window.__TAURI__ && window.__TAURI__.event;
let redraw;
if (ev) ev.listen('refreshed', () => {
  S.usageAt = Date.now(); setFresh(); call('today').then(setTodayBadge).catch(() => {});
  if (!['home', 'accounts', 'account', 'usage', 'projects', 'sessions'].includes(S.route.view)) return;
  // a dialog stays as it is; the page catches up when it closes
  if (document.getElementById('dialog-root').innerHTML) S.dirty = true;
  else if (S.route.view === 'projects') refreshProjects(); // the tiles and rows take the new numbers where they are
  else { clearTimeout(redraw); redraw = setTimeout(render, 250); } // a session being written refreshes often: one redraw per burst
});

// tools with a newer version: from the last check now, and from the check at launch when it finishes
call('state').then((st) => { applyTheme(st); setAppUpdates(st.app_updates); const v = document.getElementById('nav-version'); if (v) v.textContent = 'v' + st.version; }).catch(() => {});
// a newer Harness Manager found by an earlier check: the dot on Settings
call('self_latest').then((v) => v && setSelfUpdate(v)).catch(() => {});
// how many MCP servers and skills there are, beside their pages; the pinned projects under Projects
call('items').then((d) => setNavCounts({ 'n-mcp': d.mcp.length, 'n-skills': d.skills.length })).catch(() => {});
call('projects', { days: 30 }).then(drawPins).catch(() => {});
restoreFolds();
S.usageAt = Date.now();
if (ev) ev.listen('app-updates', (e) => setAppUpdates(e.payload));
// a new Harness Manager: Settings > Updates offers it; the hourly check also withdraws an offer that is no longer one
let told;
if (ev) ev.listen('self-update', (e) => {
  if (e.payload.version === told) return;
  told = e.payload.version;
  setSelfUpdate(e.payload);
  if (told) toast(`Harness Manager ${told} is available. Settings > Updates installs it.`);
  if (S.route.view === 'settings') render();
});

// open consoles change without any file being written; look again every minute on the accounts list
setInterval(() => { if (S.route.view === 'accounts' && !document.getElementById('dialog-root').innerHTML && document.visibilityState === 'visible') render(); }, 60000);

render();
// today's cost next to Home, also when another page opens first
call('today').then(setTodayBadge).catch(() => {});
