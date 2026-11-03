// Starts the window: icons, navigation, search, and live refresh after background imports.

import { S, render, go, closeDialog, call, setAppUpdates, setNavCounts, setFresh, applyTheme, toast } from './core.js';
import { ic, brandMark } from './lib.js';
import './accounts.js';
import { setTodayBadge } from './home.js';
import { setSelfUpdate } from './views.js';
import './mcp.js';
import './skills.js';
import './apps.js';
import './guide.js';

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

// the backend imports usage in the background and says when it is done
const ev = window.__TAURI__ && window.__TAURI__.event;
if (ev) ev.listen('refreshed', () => { S.usageAt = Date.now(); setFresh(); call('today').then(setTodayBadge).catch(() => {}); if (['home', 'accounts', 'account', 'usage'].includes(S.route.view) && !document.getElementById('dialog-root').innerHTML) render(); });

// tools with a newer version: from the last check now, and from the check at launch when it finishes
call('state').then((st) => { applyTheme(st); setAppUpdates(st.app_updates); }).catch(() => {});
// how many MCP servers and skills there are, beside their pages
call('items').then((d) => setNavCounts({ 'n-mcp': d.mcp.length, 'n-skills': d.skills.length })).catch(() => {});
S.usageAt = Date.now();
if (ev) ev.listen('app-updates', (e) => setAppUpdates(e.payload));
// a new Harness Manager: Settings > Updates offers it
if (ev) ev.listen('self-update', (e) => {
  setSelfUpdate(e.payload);
  toast(`Harness Manager ${e.payload.version} is available. Settings > Updates installs it.`);
  if (S.route.view === 'settings') render();
});

// open consoles change without any file being written; look again every minute on the accounts list
setInterval(() => { if (S.route.view === 'accounts' && !document.getElementById('dialog-root').innerHTML && document.visibilityState === 'visible') render(); }, 60000);

render();
// today's cost next to Home, also when another page opens first
call('today').then(setTodayBadge).catch(() => {});
