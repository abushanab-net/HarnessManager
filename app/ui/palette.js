// The command palette (Ctrl K, or "Run a command" in the top bar), as docs/project-review-2026-11/mockups/b-palette.html
// draws it: one box that runs an action, goes to a page or finds a session, from any page. It is a dialog of the one
// dialog controller (core.js), so the page behind is inert, a click outside or Esc closes it and the focus returns to
// where it was. An action here is a button that exists on some page, pressed: fire() sends its data-act through the
// same delegated click. Accounts, projects and the settings rows are read once when it opens; only the session
// search asks the backend, after a pause in typing.

import { S, call, go, action, openDialog, closeDialog, dialogOpen, loadState, shownTools } from './core.js';
import { esc, when, toolIcon, ic } from './lib.js';
import { PAGE } from './parts.js';
import { accountViews, metricsOf, TOOL } from './accounts.js';
import { recommendedFor } from './home.js';
import { settingRows } from './settings.js';
import { syncOn } from './computers.js';
import { tellApart } from './projects.js';

const MAX = 9; // rows on screen: one for Enter, the rest for Ctrl+2 … Ctrl+9
const MOD = /Mac|iPhone|iPad/.test(navigator.platform) ? '⌘' : 'Ctrl+';
// the open palette: what is typed, the candidates, the rows shown and the chosen one, the sessions found, the search's timer
let P = null;
// read when it opens and kept for the next time, so it opens complete while they are read again
let projects = [], settings = [];

// ---------- icons: the tool's own for a tool or an account, the sidebar's for a page, a few signs for the rest ----------
const sign = (d) => `<svg class="nav-icon" viewBox="0 0 16 16" aria-hidden="true">${d}</svg>`;
const SIGN = { search: sign('<circle cx="7" cy="7" r="4"/><path d="M10 10l3.5 3.5"/>'), plus: sign('<path d="M8 3v10M3 8h10"/>'), refresh: sign('<path d="M13 8a5 5 0 11-1.5-3.5M13 3v2.5h-2.5"/>') };
const navIcon = (id) => (document.querySelector(`[data-testid=nav-${id}] svg`) || {}).outerHTML || '';

// ---------- what it lists ----------
const PAGES = [['home', 'Use next, needs attention, running now'], ['sessions', "Every tool's sessions"], ['projects', 'Tokens and cost by project'], ['accounts', 'Limits and usage by account'],
  ['computers', 'Pairing, the hub and sync'], ['mcp', 'Setup'], ['skills', 'Setup'], ['prompts', 'Setup'], ['routing', 'Setup · providers, routes, the router'], ['apps', 'Setup · the tools and their versions'],
  ['cli', 'The hrm commands'], ['settings', 'Theme, limits, updates, backups']];

/** Every row the palette can show, in the order of use: { g (its group), icon, title, name (what a title-start match
 * is measured on; the title when absent), desc, top (listed with nothing typed), and act + data (a button's data-act
 * and data-*) or to ([view, params]) }. An action that cannot run now is not in the list. */
function build() {
  const st = S.state, pcs = S.cache.computers, tools = shownTools(st), items = [];
  const act = (icon, title, desc, name, data = {}, top = false) => items.push({ g: 'Actions', icon, title, desc, act: name, data, top });
  const to = (icon, title, desc, view, params = {}, more = {}) => items.push({ g: 'Go to', icon, title, desc, to: [view, params], ...more });
  const accts = accountViews(st, S.cache.extra || {}, Date.now(), pcs), label = (id) => TOOL[id] || id;
  // an account can start when it is logged in (or needs no login) and its tool is installed here: the rule of its page's Start
  const can = accts.filter((a) => !a.needsLogin && (tools.find((h) => h.id === a.tool) || {}).installed);
  const recs = new Map(tools.map((h) => [recommendedFor(accts, h.id), h]).filter(([a]) => can.includes(a)));
  const withRoot = projects.filter((p) => p.root), recent = [...withRoot].sort((a, b) => (b.last_active_at || 0) - (a.last_active_at || 0))[0];
  const apart = tellApart(projects); // two projects of one name are told apart, as on Sessions and Projects
  const inProject = (p, top) => act(SIGN.plus, 'New session in ' + p.name, apart.get(p.key).dup ? apart.get(p.key).hint : 'starts in its folder', 'project-new', { dir: p.root, name: p.name, profile: '' }, top);

  // ---- Actions: the useful ones first (they are the list with nothing typed)
  for (const [a, h] of recs) act(toolIcon(a.tool, 18), 'Start ' + a.name, 'recommended for ' + h.label, 'start', { name: a.id }, true);
  const best = metricsOf(accts).bestRoomAcct, first = can.includes(best) ? best : can[0];
  if (first) act(navIcon('projects'), 'Start in a folder…', '', 'start-in-folder', { name: first.id }, true);
  if (recent) inProject(recent, true);
  for (const a of can) if (!recs.has(a)) act(toolIcon(a.tool, 18), 'Start ' + a.name, label(a.tool), 'start', { name: a.id });
  for (const a of accts) if (a.needsLogin) act(toolIcon(a.tool, 18), 'Log in ' + a.name, `${label(a.tool)} · ${a.p.identity.needs_login ? 'login expired' : 'not logged in'}`, 'login', { name: a.id });
  act(SIGN.plus, 'New session', 'pick the folder and the account', 'session-new');
  for (const p of withRoot) if (p !== recent) inProject(p, false);
  act(SIGN.plus, 'Add account', 'a subscription, an API key or a routed account', 'add-account');
  act(navIcon('computers'), 'Pair a computer…', 'Computers', 'pair');
  if (syncOn(pcs)) act(navIcon('computers'), 'Sync now', 'with your other computers', 'computers-now');
  act(navIcon('settings'), 'Back up now', 'Settings › Backups', 'backup-now');
  act(SIGN.refresh, 'Refresh limits', 'read usage and limits now', 'refresh-now');
  act(navIcon('settings'), 'Check for updates', 'Harness Manager · Settings › Updates', 'self-check');
  act(navIcon('setup'), 'Check for tool updates', 'Apps', 'apps-check');
  act(navIcon('setup'), st.router && st.router.running ? 'Stop router' : 'Start router', 'API keys & routing', st.router && st.router.running ? 'router-stop' : 'router-start');
  act(navIcon('settings'), 'Find tools again', 'Settings › Maintenance', 'detect');
  act(navIcon('settings'), 'Archive now', 'old sessions · Settings › Backups', 'archive-now');
  act(navIcon('settings'), 'Open log', 'Settings › Log', 'log-open');
  act(ic.info, 'Tour of this page', '', 'tour-start');

  // ---- Go to: the pages (the list with nothing typed), then each account, project, settings section and settings row
  const live = st.profiles.reduce((n, p) => n + p.running.length, 0) + ((pcs || {}).peers || []).reduce((n, p) => n + (p.running || []).length, 0);
  for (const [id, desc] of PAGES) to(navIcon(document.querySelector(`.nav-item[data-nav=${id}]`) ? id : 'setup'), PAGE[id], id === 'sessions' && live ? `${live} running now` : desc, id, {}, { top: true });
  for (const a of accts) to(toolIcon(a.tool, 18), 'Accounts › ' + a.name, `${label(a.tool)} · Start, limits, sessions`, 'account', { name: a.id }, { name: a.name });
  for (const p of projects) to(navIcon('projects'), 'Projects › ' + p.name, [apart.get(p.key).hint, 'its sessions'].filter(Boolean).join(' · '), 'sessions', { project: p.key }, { name: p.name });
  for (const s of settings) to(navIcon('settings'), 'Settings › ' + s.title, '', 'settings', { tab: s.id }, { name: s.title });
  for (const s of settings) for (const r of s.rows) to(navIcon('settings'), 'Settings › ' + r.label, r.help, 'settings', { tab: s.id }, { name: r.label });
  return items;
}

/** A session the search found, as a row: Enter opens its page, as its title does on Sessions. */
function sessionItem(s) {
  const pcs = S.cache.computers, remote = !!(pcs && pcs.this && s.device && s.device !== pcs.this.id);
  const running = s.open ? 'running now' : s.open_elsewhere ? 'running on ' + (s.device_name || 'another computer') : '';
  return { g: 'Sessions', icon: toolIcon(s.harness, 18), title: s.title || 'Untitled session', desc: [running, s.project_name, when(s.last_active_at)].filter(Boolean).join(' · '),
    act: 'open-session', data: { profile: s.profile, id: s.id, ...(remote && !s.archived ? { device: s.device } : {}) } };
}

// ---------- matching: every typed word in the title or the description, in any case; a title that starts with the words first ----------
const wordsOf = (q) => q.toLowerCase().split(/\s+/).filter(Boolean);
/** 0: its name starts with what is typed; 1: every word is in its title; 2: a word is only in its description; -1: no match. */
function rank(it, ws) {
  const t = it.title.toLowerCase(), d = (it.desc || '').toLowerCase();
  if (!ws.every((w) => t.includes(w) || d.includes(w))) return -1;
  return (it.name || it.title).toLowerCase().startsWith(ws.join(' ')) ? 0 : ws.every((w) => t.includes(w)) ? 1 : 2;
}
/** Text as markup, the typed words marked. Titles and names are other people's text: every piece is escaped. */
function marked(text, ws) {
  const low = text.toLowerCase(), hit = new Array(text.length).fill(false);
  for (const w of ws) for (let i = low.indexOf(w); i >= 0; i = low.indexOf(w, i + w.length)) hit.fill(true, i, i + w.length);
  let out = '';
  for (let i = 0, j; i < text.length; i = j) {
    for (j = i + 1; j < text.length && hit[j] === hit[i]; j++);
    out += hit[i] ? `<mark>${esc(text.slice(i, j))}</mark>` : esc(text.slice(i, j));
  }
  return out;
}

// ---------- drawing ----------
const hint = (i) => (i === P.i ? '↵' : MOD + (i + 1));
/** Pick the rows for what is typed and draw them. Nine at most: four actions, three places and two sessions when all
 * three have that many, and a group with fewer leaves its room to the others (sessions never take more than five). */
function draw() {
  // (the candidates are built again when the projects and the settings rows arrive: a row is the same row by what it says and does)
  const ws = wordsOf(P.q), same = (it) => JSON.stringify([it.g, it.title, it.to || it.data]), was = P.rows[P.i] && same(P.rows[P.i]);
  const of = (g) => P.items.filter((it) => it.g === g).map((it) => [ws.length ? rank(it, ws) : it.top ? 0 : -1, it]).filter(([r]) => r >= 0).sort((a, b) => a[0] - b[0]).map(([, it]) => it);
  // the sessions read for an earlier word stay while they still match, until the answer for these words is here
  const found = ws.length ? P.sessions.filter((s) => ws.every((w) => s.title.toLowerCase().includes(w))) : [];
  const lists = [of('Actions'), of('Go to'), found.slice(0, 5)], n = lists.map((l, i) => Math.min(l.length, (ws.length ? [4, 3, 2] : [6, 3, 0])[i]));
  let free = MAX - n[0] - n[1] - n[2];
  lists.forEach((l, i) => { const more = Math.min(free, l.length - n[i]); n[i] += more; free -= more; });
  const total = lists[0].length + lists[1].length + found.length;
  P.rows = lists.flatMap((l, i) => l.slice(0, n[i]));
  P.i = Math.max(0, P.rows.findIndex((it) => same(it) === was)); // the chosen row stays chosen when rows arrive around it
  let i = 0;
  const row = (it) => { const k = i++; return `<div class="pal-r ${k === P.i ? 'on' : ''}" role="option" id="pal-r${k}" aria-selected="${k === P.i}" data-i="${k}" data-testid="pal-row"><span class="pal-ic">${it.icon}</span><div class="trunc"><b>${marked(it.title, ws)}</b>${it.desc ? `<span class="sub"> · ${marked(it.desc, ws)}</span>` : ''}</div><kbd class="kbd">${hint(k)}</kbd></div>`; };
  document.getElementById('pal-list').innerHTML = ['Actions', 'Go to', 'Sessions'].map((g, gi) => (n[gi] ? `<div role="group" aria-labelledby="pal-g${gi}"><div class="pal-g" id="pal-g${gi}">${g}</div>${lists[gi].slice(0, n[gi]).map(row).join('')}</div>` : '')).join('')
    || '<div class="pal-none" data-testid="pal-none">Nothing matches.</div>';
  document.getElementById('pal-count').textContent = `${P.rows.length < total ? P.rows.length + ' of ' : ''}${total} ${total === 1 ? 'result' : 'results'}`;
  point();
}
/** Tell a screen reader which row is chosen (the focus stays in the box), and keep the row in view. */
function point() {
  const q = document.getElementById('pal-q'), on = document.getElementById('pal-r' + P.i);
  if (on) { q.setAttribute('aria-activedescendant', on.id); on.scrollIntoView({ block: 'nearest' }); } else q.removeAttribute('aria-activedescendant');
}
/** Move the choice to row i, where the rows are: the ↵ moves with it. */
function choose(i) {
  if (!P.rows.length) return;
  const old = P.i;
  P.i = (i + P.rows.length) % P.rows.length;
  for (const k of [old, P.i]) {
    const el = document.getElementById('pal-r' + k);
    if (!el) continue;
    el.classList.toggle('on', k === P.i); el.setAttribute('aria-selected', k === P.i); el.querySelector('kbd').textContent = hint(k);
  }
  point();
}

// ---------- running a row ----------
/** Press the button an action belongs to: a button with its data-act and data-*, clicked where every button's click is heard. */
function fire(name, data) {
  const b = Object.assign(document.createElement('button'), { hidden: true });
  Object.assign(b.dataset, data, { act: name });
  document.body.append(b);
  b.click();
  b.remove();
}
function run(i) {
  const it = P && P.rows[i];
  if (!it) return;
  closeDialog(); // first: the focus is back where it was, so a dialog the action opens returns it there too
  if (it.to) go(...it.to); else fire(it.act, it.data);
}

// ---------- the session search: the Sessions page's own read, after a pause in typing ----------
function search() {
  const p = P, q = p.q.trim();
  clearTimeout(p.timer);
  if (q.length < 2) { p.sessions = []; return; } // one letter is in nearly every title: not worth the index's time
  p.timer = setTimeout(async () => {
    // shortcut: the read also matches a folder or an account, so the 20 newest matches are read and those with the
    // words in their title kept; a title match older than 20 others is missed. A title-only filter in the read ends that.
    const r = await call('sessions', { filter: { query: q, per: 20, page: 0 }, group: 'date', scope: 'all' }).catch(() => null);
    if (!r || P !== p || p.q.trim() !== q) return; // closed, or typed on: a later answer is the one to show
    p.sessions = r.groups.flatMap((g) => g.sessions).sort((a, b) => b.last_active_at - a.last_active_at).map(sessionItem);
    draw();
  }, 200);
}

// ---------- opening ----------
async function open() {
  if (P || dialogOpen()) return; // a dialog is modal: it is answered first
  if (!S.state) await loadState().catch(() => {});
  if (!S.state || P || dialogOpen()) return;
  document.querySelector('[data-act=tour-skip]')?.click(); // the tour has the arrow keys while it shows
  const p = P = { q: '', i: 0, items: [], rows: [], sessions: [], timer: 0 };
  const key = (k, what) => `<span><kbd class="kbd">${k}</kbd>${what}</span>`;
  const card = openDialog(`<div class="pal-in">${SIGN.search}<input id="pal-q" type="text" role="combobox" aria-expanded="true" aria-controls="pal-list" aria-autocomplete="list" aria-label="Run a command, go to a page or find a session" placeholder="Run a command, go to a page, find a session" autocomplete="off" spellcheck="false" data-testid="pal-q"><kbd class="kbd">Esc</kbd></div>
    <div class="pal-list" id="pal-list" role="listbox" aria-label="Actions, pages and sessions" data-testid="pal-list"></div>
    <div class="pal-f">${key('↑↓', 'move')}${key('↵', 'run')}${key('Esc', 'close')}<span class="grow"></span><span id="pal-count" role="status" data-testid="pal-count"></span></div>`,
  { size: 'pal', label: 'Command palette', onClose: () => { clearTimeout(p.timer); if (P === p) P = null; } });
  card.dataset.testid = 'palette';
  const list = card.querySelector('#pal-list');
  card.querySelector('#pal-q').addEventListener('input', (e) => { p.q = e.target.value; p.rows = []; search(); draw(); });
  card.addEventListener('keydown', (e) => {
    if (e.isComposing) return; // an input method's own Enter and arrows
    // the digit by its key, not its character: with Ctrl some layouts give another character (AltGr is Ctrl+Alt: not this)
    const digit = (e.ctrlKey || e.metaKey) && !e.altKey && /^(?:Digit|Numpad)([1-9])$/.exec(e.code);
    if (e.key === 'ArrowDown') choose(p.i + 1);
    else if (e.key === 'ArrowUp') choose(p.i - 1);
    else if (e.key === 'Enter') run(p.i);
    else if (digit) run(+digit[1] - 1);
    else if (e.key === 'Escape') { e.stopPropagation(); closeDialog(); } // here, so the page behind does not take the Esc too
    else return;
    e.preventDefault();
  });
  // the pointer chooses a row when it moves, not when rows are drawn under it while it rests
  let at = '';
  list.addEventListener('mousemove', (e) => {
    const r = e.target.closest('.pal-r'), now = e.screenX + ',' + e.screenY;
    if (r && at && now !== at && +r.dataset.i !== p.i) choose(+r.dataset.i);
    at = now;
  });
  list.addEventListener('mousedown', (e) => e.preventDefault()); // the box keeps the focus
  list.addEventListener('click', (e) => { const r = e.target.closest('.pal-r'); if (r) run(+r.dataset.i); });
  // what is kept draws at once; the projects and the settings rows are read once and join the list when they arrive
  const fill = () => { if (P === p) { p.items = build(); draw(); } };
  fill();
  settingRows().then((s) => { settings = s; fill(); }).catch(() => {});
  call('projects', { days: 30 }).then((l) => { projects = l; fill(); }).catch(() => {});
}
action('palette', () => open());
document.addEventListener('keydown', (e) => {
  if ((e.key || '').toLowerCase() !== 'k' || !(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return;
  e.preventDefault();
  if (P) closeDialog(); else open();
});
// the hint beside "Run a command" names the key of this system
for (const k of document.querySelectorAll('[data-act=palette] kbd')) k.textContent = MOD === '⌘' ? '⌘ K' : 'Ctrl K';
