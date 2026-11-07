// Starts the window: icons, navigation, search, and live refresh after background imports.

import { S, render, go, call, setAppUpdates, setNavCounts, setFresh, applyTheme, toast, drawPins, restoreFolds } from './core.js';
import { brandMark } from './lib.js';
import './accounts.js';
import { setTodayBadge } from './home.js';
import { setSelfUpdate } from './settings.js';
import { refreshProjects } from './projects.js';
import './sessions.js';
import './routing.js';
import './prompts.js';
import './cli.js';
import './mcp.js';
import './skills.js';
import './apps.js';
import './guide.js';
import './computers.js';

document.documentElement.dataset.theme = 'dark';
document.getElementById('brand-mark').innerHTML = brandMark(28);

// moving to another page draws it (core.js closes the dialog, the menu and the notice of the page left)
window.addEventListener('hashchange', () => render());

const find = document.getElementById('find');
find.addEventListener('input', () => { if (S.route.view === 'accounts') render(); else if (S.pageFind) S.pageFind(find.value); });
find.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && S.route.view !== 'accounts' && !S.pageFind) { go('sessions', { q: find.value }); find.value = ''; }
});

// the backend imports usage in the background, and the folder watcher refreshes the index, and both say when done
const ev = window.__TAURI__ && window.__TAURI__.event;
// A session being written refreshes about once a second, and a page's reads take seconds on a busy database, so the
// redraws are coalesced: one at a time, at least four seconds apart, and one more afterwards if events kept coming.
// Queueing a render per event made the renders pile up behind each other and held every other call for half a minute.
let redraw, drawing = false, again = false, lastDraw = 0;
const redrawSoon = () => {
  clearTimeout(redraw);
  redraw = setTimeout(async () => {
    if (drawing) { again = true; return; }
    drawing = true;
    try {
      call('today').then(setTodayBadge).catch(() => {});
      if (document.getElementById('dialog-root').innerHTML) S.dirty = true; // a dialog stays as it is; the page catches up when it closes
      else if (S.route.view === 'projects') await refreshProjects(); // the tiles and rows take the new numbers where they are
      else await render();
    } finally { drawing = false; lastDraw = Date.now(); if (again) { again = false; redrawSoon(); } }
  }, Math.max(250, 4000 - (Date.now() - lastDraw)));
};
if (ev) ev.listen('refreshed', () => {
  S.usageAt = Date.now(); setFresh();
  if (['home', 'accounts', 'account', 'usage', 'projects', 'sessions'].includes(S.route.view)) redrawSoon();
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

// the limits popup by the tray icon opens this window at an account
if (ev) ev.listen('navigate', (e) => { location.hash = e.payload; });

render();
// today's cost beside Overview, also when another page opens first
call('today').then(setTodayBadge).catch(() => {});
