// Shared runtime: backend calls, app state, navigation, the keyed reconciler that redraws a page in place, the one
// dialog controller, the popover menu, the one notice channel, theme, the sidebar's folds and pins.

import { esc, dur } from './lib.js';

export const S = { state: null, route: { view: 'accounts', params: {} }, cache: {}, timers: [] };

/** Call the backend. Every operation goes through one Tauri command. */
export async function call(op, args = {}) {
  const t = window.__TAURI__;
  if (!t) throw new Error('Harness Manager must run inside its desktop window');
  return t.core.invoke('call', { op, args });
}

/** The number of tools with a newer version, beside Setup and Apps in the rail. */
export function setAppUpdates(n) {
  for (const el of document.querySelectorAll('#apps-count, #n-apps')) {
    el.textContent = n ? `${n} ${n === 1 ? 'update' : 'updates'}` : '';
    el.title = n ? `${n} ${n === 1 ? 'tool has' : 'tools have'} a newer version: Apps installs them` : '';
  }
}

/** Live sessions as a 4×4 grid of dots with n of them lit (the number on hover); nothing when none. */
// drawn as SVG squares snapped to device pixels, so a lit one is a flat colour, not a blurred glow at 125 % scaling
export const liveGrid = (n) => (n ? `<svg class="lgrid" width="11" height="11" viewBox="0 0 11 11" shape-rendering="crispEdges" role="img" aria-label="${n} live"><title>${n} live</title>${Array.from({ length: 16 }, (_, i) => `<rect x="${(i % 4) * 3}" y="${Math.floor(i / 4) * 3}" width="2" height="2" class="${i < n ? 'on' : ''}"/>`).join('')}</svg>` : '');

/** The pinned projects under Projects in the rail (at most five), and the project count. */
export function drawPins(list) {
  const el = document.getElementById('nav-pins');
  if (!el) return;
  const cur = S.route.view === 'sessions' ? S.route.params.project : '';
  el.innerHTML = list.filter((p) => p.pinned).slice(0, 5).map((p) => `<a class="nav-subitem ${p.key === cur ? 'active' : ''}" href="#sessions?project=${encodeURIComponent(p.key)}" data-nav-project="${esc(p.key)}" title="${esc(p.remote || p.root || p.name)}" data-testid="pin-${esc(p.key)}"><span>${esc(p.name)}</span><span class="nav-badge">${p.live ? `<span class="live-n">●${p.live}</span>` : p.sessions || ''}<span class="unpin" data-act="pin-off" data-key="${esc(p.key)}" role="button" tabindex="0" title="Unpin ${esc(p.name)}" aria-label="Unpin ${esc(p.name)}" data-testid="unpin-${esc(p.key)}">✕</span></span></a>`).join('');
  setNavCounts({ 'n-projects': String(list.length || '') });
}

/** The counts beside the pages in the rail; each one only when known. Our own markup: a number, or a number with a live grid. */
export function setNavCounts(c) {
  for (const [id, v] of Object.entries(c)) { const el = document.getElementById(id); if (el) el.innerHTML = v || ''; }
}

/** "Limits read 2 m ago · usage 1 m ago" in the top bar, on every page. */
export function setFresh() {
  const st = S.state, el = document.getElementById('fresh');
  if (!st || !el) return;
  const now = Date.now(), ago = (t) => (now - t < 60000 ? 'just now' : dur(now - t) + ' ago');
  const read = st.profiles.map((p) => p.quota && p.quota.fetched_at).filter(Boolean);
  el.textContent = [read.length ? 'Limits read ' + ago(Math.max(...read)) : 'Limits not read', S.usageAt ? 'usage ' + ago(S.usageAt) : ''].filter(Boolean).join(' · ');
}

/** The name an account is shown by (its label), from its id; the id when unknown. */
export const acctName = (id) => ((S.state && S.state.profiles) || []).find((p) => p.name === id)?.label || id;

/** Draw a chart for the width its box has now, and again whenever the window changes size (maximize, restore). */
const fits = new Set();
export function fit(el, draw) {
  const f = () => { el.innerHTML = draw(el.clientWidth); };
  f();
  for (const x of fits) if (!x[0].isConnected) fits.delete(x); // pages redraw often: forget charts no longer shown
  fits.add([el, f]);
}
let fitTimer;
window.addEventListener('resize', () => {
  clearTimeout(fitTimer);
  fitTimer = setTimeout(() => { for (const x of fits) { if (x[0].isConnected) x[1](); else fits.delete(x); } }, 120);
});

/** What a refresh did: new records, a live read that failed, and why Claude limits can stay old. */
export function refreshText(r) {
  const now = Date.now(), st = S.state || {};
  // with Live off, Claude limits are only Claude Code's own saved copies, which it rarely updates
  const old = !st.live_quota && (st.profiles || []).some((p) => p.harness === 'claude' && p.kind === 'subscription' && p.identity.logged_in
    && !(p.quota && p.quota.fetched_at && now - p.quota.fetched_at < 10 * 60000));
  return [r.new_rows ? `${r.new_rows.toLocaleString()} new ${r.new_rows === 1 ? 'record' : 'records'}` : 'Usage is up to date',
    r.quota_errors.length ? `Limits of ${acctName(r.quota_errors[0][0])}: ${r.quota_errors[0][1]}` : '',
    old ? 'Claude limits are the copies Claude Code last saved, which can be hours old. Turn on Live Claude limits in Settings to read current ones' : '',
  ].filter(Boolean).join('. ') + '.';
}

// ---------- the one notice channel: the bar under the top bar; cleared when the page changes ----------
let toastTimer;
function bar(text, { warn = false, toast: isToast = false, act = null, id } = {}) {
  // an open dialog covers the page's bar: a failure in it is said in it, above its buttons
  const card = warn && document.querySelector('#dialog-root .modal-card');
  if (card) document.getElementById('notice').innerHTML = '';
  const el = card ? (card.querySelector('.dlg-notice') || card.insertBefore(Object.assign(document.createElement('div'), { className: 'dlg-notice' }), card.querySelector('.modal-foot'))) : document.getElementById('notice');
  if (!el) return;
  clearTimeout(toastTimer);
  el.innerHTML = `<div class="notice-bar ${warn ? 'warn' : ''} ${isToast ? 'toast' : ''}" role="${warn ? 'alert' : 'status'}"><span id="${id}">${esc(text)}</span>${act ? `<button class="filter-chip notice-act" data-act="${esc(act.act)}" ${Object.entries(act.data || {}).map(([k, v]) => `data-${k}="${esc(v)}"`).join(' ')} data-testid="notice-act">${esc(act.label)}</button>` : ''}<button class="filter-chip" data-act="notice-close" aria-label="Dismiss">Dismiss</button></div>`;
}
/** A short confirmation after an action, or an error; it stays long enough to read (about 60 ms a character), then goes. */
export function toast(msg, isErr = false) {
  bar(msg, { warn: isErr, toast: true, id: 'toast' });
  toastTimer = setTimeout(clearNotice, Math.max(isErr ? 7000 : 3500, msg.length * 60));
}
/** A failure that stays on screen until dismissed, with an optional offer beside it; the next failure replaces it. */
export function notice(msg, act) { bar(msg, { warn: true, act, id: 'notice-text' }); }
export function clearNotice() { clearTimeout(toastTimer); for (const el of document.querySelectorAll('#notice, .dlg-notice')) el.innerHTML = ''; }

/** Run an action; show its error instead of failing silently. */
export async function attempt(fn, okMsg) {
  try {
    const r = await fn();
    if (okMsg) toast(okMsg);
    return r;
  } catch (e) {
    notice(String(e).replace(/^Error: /, ''));
    return undefined;
  }
}

/** Something changed on disk: forget the cached state, refresh the rail's counts, and draw the page again. */
export async function changed() {
  await Promise.all([loadState().catch(() => {}), call('items').then((d) => setNavCounts({ 'n-mcp': d.mcp.length, 'n-skills': d.skills.length })).catch(() => {}),
    call('projects', { days: 30 }).then(drawPins).catch(() => {})]);
  return render();
}

/** A change the user asked for: run it, say what happened, and draw the page again whatever the outcome,
 * so the page never shows a state older than what is on disk. Resolves to the call's result, or undefined. */
export async function mutate(fn, okMsg) {
  const r = await attempt(fn, okMsg);
  await changed();
  return r;
}

// ---------- the router ----------
const routes = {};
export function view(name, render) { routes[name] = render; }

export function go(view, params = {}) {
  location.hash = '#' + view + (Object.keys(params).length ? '?' + new URLSearchParams(params) : '');
}

export function parseHash() {
  const h = location.hash.slice(1) || 'home';
  const [v, q] = h.split('?');
  return { view: v, params: Object.fromEntries(new URLSearchParams(q || '')) };
}

// renders run one after another, so a slow earlier page never paints over the one you moved to
let chain = Promise.resolve(), queued = null;
export function render() {
  // one redraw waits at most: a burst of refreshes while a page loads draws it once more, not once per event
  if (!queued) {
    const run = () => { queued = null; return renderNow(); };
    queued = chain = chain.then(run, run);
  }
  return queued;
}

async function renderNow() {
  // a redraw of the same page (a refresh, a filter) keeps its scroll position; moving to another page starts at the top.
  // The position is the one the sheet had when its content was replaced, not when the redraw began: a page's data takes
  // seconds to load, and restoring the older position pulled the reader back up while they scrolled
  const sheet0 = document.getElementById('sheet'), sameHash = S.lastHash === location.hash;
  if (sheet0 && !S.sheetWatched) { S.sheetWatched = true; S.sheetTop = sheet0.scrollTop; sheet0.addEventListener('scroll', () => { S.sheetTop = sheet0.scrollTop; }, { passive: true }); }
  S.lastHash = location.hash;
  S.route = parseHash();
  S.pageFind = null; // a page that lists things sets its own filter for the top bar's search
  document.querySelectorAll('.nav a[data-nav]').forEach((a) => {
    const on = a.dataset.nav === S.route.view || (a.dataset.nav === 'accounts' && S.route.view === 'account') || (a.dataset.nav === 'sessions' && S.route.view === 'session') || (a.dataset.nav === 'mcp' && S.route.view === 'mcp-edit');
    a.classList.toggle('active', on);
    a.classList.toggle('on', on);
    if (on) a.setAttribute('aria-current', 'page'); else a.removeAttribute('aria-current');
  });
  document.querySelectorAll('.nav a[data-nav-project]').forEach((a) => a.classList.toggle('active', S.route.view === 'sessions' && a.dataset.navProject === S.route.params.project));
  // a page inside a shut fold of the rail opens the fold
  const fold = document.querySelector('.nav a[data-nav].active')?.closest('.nav-sublist');
  if (fold && fold.hidden) setFold(fold.id, false);
  const fn = routes[S.route.view] || routes.home;
  const sheet = document.getElementById('sheet');
  if (!sameHash) { delete sheet.dataset.warm; sheet.scrollTop = 0; }
  try {
    await fn(sheet, S.route.params);
    const top = S.sheetTop; // what the reader had before fn replaced the content; the replacement's own scroll event has not fired yet
    if (sameHash && top) { sheet.scrollTop = top; requestAnimationFrame(() => { if (S.lastHash === location.hash) sheet.scrollTop = top; }); }
    // pages can add to a freshly drawn sheet (the tour in guide.js)
    document.dispatchEvent(new CustomEvent('rendered', { detail: { sheet, view: routes[S.route.view] ? S.route.view : 'home' } }));
  } catch (e) {
    sheet.className = 'sheet';
    sheet.innerHTML = `<div class="page-body"><section class="panel-card empty-state"><h1>Something went wrong</h1><span class="negT">${esc(String(e))}</span><div><button class="btn-secondary" data-act="reload">Try again</button></div></section></div>`;
  }
}

// ---------- the keyed reconciler: a page redraws in place, keeping what the reader had ----------
// Children are matched by id, data-key or data-testid (else by position and tag), so a redraw after a refresh keeps the
// existing elements: scroll positions, the focused control with its caret and selection, open folds, and the animated
// nodes with their phase. A page draws with patch(sheet, html) instead of sheet.innerHTML = html.
const nodeKey = (n) => (n.nodeType !== 1 ? null : n.id ? '#' + n.id : n.dataset.key ? 'k:' + n.dataset.key : n.dataset.testid ? 't:' + n.dataset.testid : null);
function patchNode(cur, next) {
  if (cur.nodeType !== next.nodeType || cur.nodeName !== next.nodeName) { const r = next.cloneNode(true); cur.replaceWith(r); return r; }
  if (cur.nodeType !== 1) { if (cur.nodeValue !== next.nodeValue) cur.nodeValue = next.nodeValue; return cur; }
  for (const a of [...cur.attributes]) if (!next.hasAttribute(a.name)) cur.removeAttribute(a.name);
  for (const a of [...next.attributes]) if (cur.getAttribute(a.name) !== a.value) cur.setAttribute(a.name, a.value);
  const keyed = new Map();
  for (const c of cur.childNodes) { const k = nodeKey(c); if (k && !keyed.has(k)) keyed.set(k, c); }
  let cursor = cur.firstChild;
  for (const child of [...next.childNodes]) {
    const k = nodeKey(child);
    let m = k ? keyed.get(k) : cursor && !nodeKey(cursor) && cursor.nodeName === child.nodeName ? cursor : null;
    if (m && k) keyed.delete(k);
    if (!m) { m = child.cloneNode(true); cur.insertBefore(m, cursor); }
    else { if (m !== cursor) cur.insertBefore(m, cursor); m = patchNode(m, child); }
    cursor = m.nextSibling;
  }
  while (cursor) { const n = cursor.nextSibling; cursor.remove(); cursor = n; }
  if (cur instanceof HTMLInputElement) {
    if (cur.type === 'checkbox' || cur.type === 'radio') { if (document.activeElement !== cur) cur.checked = next.checked; }
    else if (document.activeElement !== cur && cur.value !== next.value) cur.value = next.value;
  }
  if ((cur instanceof HTMLSelectElement || cur instanceof HTMLTextAreaElement) && document.activeElement !== cur && cur.value !== next.value) cur.value = next.value;
  return cur;
}
const focusKey = (el) => (el && el !== document.body ? (el.id ? '#' + CSS.escape(el.id) : el.dataset.testid ? `[data-testid="${CSS.escape(el.dataset.testid)}"]` : el.dataset.key ? `[data-key="${CSS.escape(el.dataset.key)}"]` : null) : null);
export function patch(root, html) {
  const scrolls = [...root.querySelectorAll('*')].filter((el) => el.scrollTop || el.scrollLeft).map((el) => [el, el.scrollTop, el.scrollLeft]);
  const active = document.activeElement, sel = root.contains(active) ? focusKey(active) : null;
  const range = active && typeof active.selectionStart === 'number' ? [active.selectionStart, active.selectionEnd, active.selectionDirection] : null;
  const t = document.createElement('template');
  t.innerHTML = html;
  const next = document.createElement(root.tagName);
  next.append(t.content);
  for (const a of [...root.attributes]) next.setAttribute(a.name, a.value);
  patchNode(root, next);
  for (const [el, top, left] of scrolls) if (el.isConnected) { el.scrollTop = top; el.scrollLeft = left; }
  if (sel && !root.contains(document.activeElement)) {
    const el = root.querySelector(sel);
    if (el) { el.focus({ preventScroll: true }); if (range && typeof el.setSelectionRange === 'function') { try { el.setSelectionRange(range[0], range[1], range[2]); } catch {} } }
  }
}

/** The tools the user shows (Apps > Show); hidden ones are left out of every page. */
export const shownTools = (st) => st.harnesses.filter((h) => !((st.settings && st.settings.hidden_tools) || []).includes(h.id));

/** The brand (Harness or VektraGrid) and light or dark; "system" follows Windows or macOS, and changes with it.
 * The stylesheet reads html[data-brand][data-theme], always one of the four. */
const dark = window.matchMedia('(prefers-color-scheme: dark)');
let themeSetting = 'system';
const setTheme = () => { document.documentElement.dataset.theme = themeSetting === 'system' ? (dark.matches ? 'dark' : 'light') : themeSetting; };
dark.addEventListener('change', setTheme);
export function applyTheme(st) {
  if (!st || !st.settings) return;
  themeSetting = st.settings.theme || 'system';
  document.documentElement.dataset.brand = st.settings.brand === 'vg' ? 'vg' : 'hm';
  document.documentElement.dataset.motion = st.settings.moving_routes === false ? 'still' : 'live';
  setTheme();
}

/** The live count in the rail and the chip in the top bar: this computer's running sessions plus every other computer's, from their presence. */
export function redrawLive() {
  if (!S.state) return;
  const running = S.state.profiles.flatMap((p) => p.running.map((r) => ({ ...r, profile: p.name })));
  const elsewhere = ((S.cache.computers || {}).peers || []).reduce((a, p) => a + (p.running || []).length, 0);
  const live = running.length + elsewhere;
  setNavCounts({ 'n-live': live ? liveGrid(live) + `<span>${live}</span>` : '' });
  const strip = document.getElementById('live-strip');
  if (strip) {
    strip.hidden = !live;
    strip.querySelector('.live-n').textContent = `${running.length} live · ${elsewhere} elsewhere`;
    strip.href = '#sessions?live=1';
  }
}

/** A page's last markup, shown at once while fresh data is read; the view calls keep() after it draws. */
// the page already on screen is never swapped for its own stale copy: its routes keep moving while the fresh data loads
export function warmPage(sheet, key) { if (sheet.dataset.warm === key) return true; const h = S.cache.html && S.cache.html[key]; if (!h) return false; sheet.innerHTML = h; sheet.dataset.warm = key; return true; }
export function keepPage(sheet, key) {
  S.cache.html ||= {};
  delete S.cache.html[key]; // to the end: the most recent pages are the ones kept
  S.cache.html[key] = sheet.innerHTML; sheet.dataset.warm = key;
  // kept for the next launch too (the last eight pages), so the first page opens drawn while its data loads
  try { const all = Object.entries(S.cache.html).slice(-8); localStorage.setItem('hm.pages', JSON.stringify(Object.fromEntries(all))); } catch {}
}
try { S.cache.html = JSON.parse(localStorage.getItem('hm.pages') || '{}') || {}; } catch { S.cache.html = {}; }

export async function loadState() {
  S.state = await call('state');
  applyTheme(S.state);
  const n = S.state.profiles.length;
  document.getElementById('brand-sub').textContent = `${S.state.computer} · ${n} ${n === 1 ? 'account' : 'accounts'}`;
  setNavCounts({ 'n-accounts': String(n) });
  redrawLive();
  setFresh();
  return S.state;
}

// ---------- the dialog controller: one modal at a time, focus trapped inside and restored to its opener ----------
const frame = () => document.querySelector('.frame');
let opener = null;
export function openDialog(html, { onClose, size = '', label = '' } = {}) {
  closeDialog();
  closeMenu();
  opener = document.activeElement && document.activeElement !== document.body ? document.activeElement : null;
  const root = document.getElementById('dialog-root');
  root.innerHTML = `<div class="modal-backdrop" data-act="dialog-close"><div class="modal-card dlg ${size}" role="dialog" aria-modal="true" ${label ? `aria-label="${esc(label)}"` : ''} tabindex="-1">${html}</div></div>`;
  root._onClose = onClose;
  frame().inert = true;
  focusFirst(root.querySelector('.modal-card'));
  return root.querySelector('.modal-card');
}
const FOCUSABLE = 'button:not([disabled]), input:not([disabled]):not([type=hidden]), select:not([disabled]), textarea:not([disabled]), a[href], [tabindex="0"]';
function focusFirst(card) {
  if (!card) return;
  const first = card.querySelector('input:not([type=hidden]):not([readonly]):not([disabled]), select:not([disabled]), textarea:not([disabled]), .tool-pick-card.active, button.btn-primary:not([disabled])') || card.querySelector(FOCUSABLE);
  (first || card).focus({ preventScroll: true });
}

export function closeDialog() {
  const root = document.getElementById('dialog-root');
  if (!root.innerHTML) return;
  if (root._onClose) { const f = root._onClose; root._onClose = null; f(); }
  root.innerHTML = '';
  frame().inert = false;
  if (opener && opener.isConnected) opener.focus({ preventScroll: true });
  opener = null;
  // a background refresh that arrived while the dialog was open catches up now
  if (S.dirty) { S.dirty = false; render(); }
}
export const dialogOpen = () => !!document.getElementById('dialog-root').innerHTML;

export const dlgHead = (title) => `<div class="modal-head dh"><h2>${esc(title)}</h2><button class="icon-btn" data-act="dialog-close" aria-label="Close">✕</button></div>`;

/** Ask before something that cannot be undone. Resolves true when confirmed. */
export function confirmDialog(title, body, okLabel = 'Confirm', danger = true) {
  return new Promise((resolve) => {
    let done = false;
    openDialog(`${dlgHead(title)}<div class="dlg-text">${body}</div>
      <div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button><button class="${danger ? 'btn-secondary negT' : 'btn-primary'}" id="confirm-ok">${esc(okLabel)}</button></span></div>`,
    { onClose: () => { if (!done) resolve(false); }, size: 'sm', label: title });
    document.getElementById('confirm-ok').onclick = () => { done = true; closeDialog(); resolve(true); };
  });
}

// ---------- the popover menu under a ⋯ ----------
let menuEl = null, menuAnchor = null, outside = null;
export function openMenu(anchor, items) {
  closeMenu();
  const r = anchor.getBoundingClientRect();
  const m = document.createElement('div');
  m.className = 'menu';
  m.setAttribute('role', 'menu');
  // an item may carry its own html (an icon or a dot before the label), a testid, and `on` for the current choice
  m.innerHTML = items.map((it, i) => `<button type="button" role="menuitem" data-i="${i}" class="${it.danger ? 'danger' : ''} ${it.on ? 'on' : ''}" ${it.on ? 'aria-current="true"' : ''} ${it.testid ? `data-testid="${esc(it.testid)}"` : ''}>${it.html || esc(it.label)}</button>`).join('');
  document.body.appendChild(m);
  menuEl = m; menuAnchor = anchor;
  anchor.setAttribute('aria-expanded', 'true');
  const left = Math.min(r.left, window.innerWidth - m.offsetWidth - 8);
  m.style.left = left + 'px';
  // near the bottom of the window the menu opens upward, so every item stays reachable
  m.style.top = (r.bottom + 4 + m.offsetHeight > window.innerHeight - 8 ? Math.max(8, r.top - 4 - m.offsetHeight) : r.bottom + 4) + 'px';
  m.onclick = (e) => { const b = e.target.closest('button[data-i]'); if (b) { closeMenu(); items[+b.dataset.i].run(); } };
  m.onkeydown = (e) => {
    const bs = [...m.querySelectorAll('button')], i = bs.indexOf(document.activeElement);
    if (e.key === 'ArrowDown') { e.preventDefault(); bs[(i + 1) % bs.length].focus(); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); bs[(i - 1 + bs.length) % bs.length].focus(); }
    else if (e.key === 'Tab') { e.preventDefault(); closeMenu(); }
  };
  m.querySelector('button')?.focus();
  setTimeout(() => { outside = (e) => { if (!m.contains(e.target)) closeMenu(); }; document.addEventListener('click', outside); }, 0);
}
export function closeMenu() {
  if (outside) { document.removeEventListener('click', outside); outside = null; }
  if (menuEl) { menuEl.remove(); menuEl = null; }
  if (menuAnchor) { menuAnchor.removeAttribute('aria-expanded'); if (menuAnchor.isConnected) menuAnchor.focus({ preventScroll: true }); menuAnchor = null; }
}

// ---------- actions by data-act ----------
const actions = {
  'dialog-close': () => closeDialog(),
  'notice-close': () => clearNotice(),
  reload: () => render(),
};
export function action(name, fn) { actions[name] = fn; }

document.addEventListener('click', (e) => {
  const el = e.target.closest('[data-act]');
  if (!el) return;
  // the backdrop closes the dialog only when the click is on it, not inside the card
  if (el.classList.contains('modal-backdrop') && e.target !== el) return;
  const fn = actions[el.dataset.act];
  // a dropdown acts when its choice changes (below), not when it is clicked open
  if (el.tagName === 'SELECT') return;
  // checkboxes keep their default (toggling); everything else is handled here
  if (fn) { if (!(el.tagName === 'INPUT' && el.type === 'checkbox')) e.preventDefault(); fn(el, e); }
});
document.addEventListener('change', (e) => {
  const el = e.target.closest && e.target.closest('select[data-act]');
  if (el && actions[el.dataset.act]) actions[el.dataset.act](el, e);
});

/** The open modal or tray, whose Tab order is trapped. */
const trapRoot = () => document.querySelector('#dialog-root .modal-card') || document.querySelector('#tray-root .tray-popup');
document.addEventListener('keydown', (e) => {
  const trap = trapRoot();
  if (e.key === 'Tab' && trap) {
    const f = [...trap.querySelectorAll(FOCUSABLE)].filter((x) => x.offsetParent !== null || x === trap);
    if (f.length) {
      const first = f[0], last = f[f.length - 1];
      if (e.shiftKey && (document.activeElement === first || document.activeElement === trap)) { e.preventDefault(); last.focus(); }
      else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
      else if (!trap.contains(document.activeElement)) { e.preventDefault(); first.focus(); }
    }
    return;
  }
  if (e.key === 'Escape') {
    if (dialogOpen()) { closeDialog(); return; }
    if (menuEl) { closeMenu(); return; }
    if (document.querySelector('#tray-root .tray-popup')) { actions['toggle-tray']?.(document.querySelector('[data-act=toggle-tray]')); return; }
  }
  if (e.key === 'k' && (e.ctrlKey || e.metaKey)) { e.preventDefault(); document.getElementById('find').focus(); }
  // a non-button element acting as one (role="button") answers Enter and Space like a button
  const rb = e.target.closest && e.target.closest('[role=button][data-act], [tabindex="0"][data-act]');
  if (rb && rb.tagName !== 'BUTTON' && rb.tagName !== 'INPUT' && rb.tagName !== 'SELECT' && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); rb.click(); }
});
// moving to another page: the dialog, the menu and the notice of the page left go with it
window.addEventListener('hashchange', () => { closeDialog(); closeMenu(); clearNotice(); });

/** A folder picker from the dialog plugin; resolves to a path or null. */
export async function pickFolder() {
  const d = window.__TAURI__ && window.__TAURI__.dialog;
  if (!d) return null;
  return await d.open({ directory: true, multiple: false });
}

export async function saveFile(defaultPath, filter = { name: 'Markdown', extensions: ['md'] }) {
  const d = window.__TAURI__ && window.__TAURI__.dialog;
  if (!d) return null;
  return await d.save({ defaultPath, filters: [filter] });
}

/** A file picker for one file of a kind; resolves to a path or null. */
export async function pickFile(filter) {
  const d = window.__TAURI__ && window.__TAURI__.dialog;
  if (!d) return null;
  return await d.open({ multiple: false, directory: false, filters: [filter] });
}

// ---------- the rail's folds and pins ----------
// the pinned projects and the Setup pages fold; the shut ones are remembered on this computer (Setup starts shut)
const FOLD_KEY = 'hm.nav.shut';
const shutFolds = () => { try { const v = JSON.parse(localStorage.getItem(FOLD_KEY)); return Array.isArray(v) ? v : ['nav-setup']; } catch { return ['nav-setup']; } };
export function setFold(id, shut, save = true) {
  const f = document.getElementById(id), head = document.querySelector(`[data-fold="${id}"]`);
  if (!f || !head) return;
  f.hidden = shut;
  head.classList.toggle('shut', shut);
  head.setAttribute('aria-expanded', String(!shut));
  if (save) { const v = shutFolds().filter((x) => x !== id); if (shut) v.push(id); try { localStorage.setItem(FOLD_KEY, JSON.stringify(v)); } catch {} }
}
export function restoreFolds() { const shut = shutFolds(); for (const id of ['nav-pins', 'nav-setup']) setFold(id, shut.includes(id), false); }
action('nav-fold', (el) => setFold(el.dataset.fold, !document.getElementById(el.dataset.fold).hidden));
// the ✕ beside a pinned project in the rail
action('pin-off', async (el) => {
  await attempt(() => call('project_pin', { key: el.dataset.key, on: false }));
  call('projects', { days: 30 }).then(drawPins).catch(() => {});
  document.dispatchEvent(new CustomEvent('hm:unpinned', { detail: el.dataset.key }));
});
