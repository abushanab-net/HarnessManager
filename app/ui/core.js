// Shared runtime: backend calls, app state, navigation, toasts, dialogs and menus.

import { esc, ic, dur } from './lib.js';

export const S = { state: null, route: { view: 'accounts', params: {} }, cache: {}, timers: [] };

/** Call the backend. Every operation goes through one Tauri command. */
export async function call(op, args = {}) {
  const t = window.__TAURI__;
  if (!t) throw new Error('Harness Manager must run inside its desktop window');
  return t.core.invoke('call', { op, args });
}

/** The number of tools with a newer version, beside Apps in the sidebar. */
export function setAppUpdates(n) {
  const el = document.getElementById('apps-count');
  if (el) el.textContent = n ? `${n} ${n === 1 ? 'update' : 'updates'}` : '';
}

/** The pinned projects under Projects in the sidebar (at most five), and the project count. */
export function drawPins(list) {
  const el = document.getElementById('nav-pins');
  if (!el) return;
  const cur = S.route.view === 'sessions' ? S.route.params.project : '';
  el.innerHTML = list.filter((p) => p.pinned).slice(0, 5).map((p) => `<a class="sub ${p.key === cur ? 'on' : ''}" href="#sessions?project=${encodeURIComponent(p.key)}" data-nav-project="${esc(p.key)}" title="${esc(p.remote || p.root || p.name)}" data-testid="pin-${esc(p.key)}"><i class="dot ${p.live ? 'live' : ''}"></i><span>${esc(p.name)}</span><span class="count ${p.live ? 'live' : ''}">${p.live ? p.live + ' live' : p.sessions || ''}</span></a>`).join('');
  setNavCounts({ 'n-projects': list.length });
}

/** The counts beside the pages in the sidebar; each one only when known. */
export function setNavCounts(c) {
  for (const [id, v] of Object.entries(c)) { const el = document.getElementById(id); if (el) el.textContent = v || ''; }
}

/** "Limits read 2 m ago · usage 1 m ago" in the top bar, on every page. */
export function setFresh() {
  const st = S.state, el = document.getElementById('fresh');
  if (!st || !el) return;
  const now = Date.now(), ago = (t) => (now - t < 60000 ? 'just now' : dur(now - t) + ' ago');
  const read = st.profiles.map((p) => p.quota && p.quota.fetched_at).filter(Boolean);
  el.textContent = [read.length ? 'Limits read ' + ago(Math.max(...read)) : '', S.usageAt ? 'usage ' + ago(S.usageAt) : ''].filter(Boolean).join(' · ');
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

let toastTimer;
export function toast(msg, isErr = false) {
  const el = document.getElementById('toast');
  el.textContent = msg;
  el.className = isErr ? 'err' : '';
  clearTimeout(toastTimer);
  // long enough to read: about 60 ms a character
  toastTimer = setTimeout(() => { el.className = 'off'; el.textContent = ''; }, Math.max(isErr ? 7000 : 3500, msg.length * 60));
}

/** Run an action; show its error as a toast instead of failing silently. */
export async function attempt(fn, okMsg) {
  try {
    const r = await fn();
    if (okMsg) toast(okMsg);
    return r;
  } catch (e) {
    const msg = String(e).replace(/^Error: /, '');
    toast(msg, true);
    notice(msg);
    return undefined;
  }
}

/** A failure that stays on screen until dismissed, unlike a toast; the next failure replaces it. */
export function notice(msg, act) {
  const el = document.getElementById('notice');
  if (!el) return;
  const t = document.getElementById('notice-text');
  t.textContent = msg;
  // an offer beside the words: what to do about it, as a button with its own action
  el.querySelector('.btn')?.remove();
  if (act) t.insertAdjacentHTML('afterend', `<button class="btn sm" data-act="${act.act}" ${Object.entries(act.data || {}).map(([k, v]) => `data-${k}="${esc(v)}"`).join(' ')} data-testid="notice-act">${esc(act.label)}</button>`);
  el.className = '';
}

/** Something changed on disk: forget the cached state, refresh the sidebar's counts, and draw the page again. */
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
let chain = Promise.resolve();
export function render() {
  chain = chain.then(renderNow, renderNow);
  return chain;
}

async function renderNow() {
  S.route = parseHash();
  S.pageFind = null; // a page that lists things sets its own filter for the top bar's search
  document.querySelectorAll('nav.nav a[data-nav]').forEach((a) => {
    const on = a.dataset.nav === S.route.view || (a.dataset.nav === 'accounts' && S.route.view === 'account') || (a.dataset.nav === 'sessions' && S.route.view === 'session') || (a.dataset.nav === 'mcp' && S.route.view === 'mcp-edit');
    a.classList.toggle('on', on);
    if (on) a.setAttribute('aria-current', 'page'); else a.removeAttribute('aria-current');
  });
  document.querySelectorAll('nav.nav a[data-nav-project]').forEach((a) => a.classList.toggle('on', S.route.view === 'sessions' && a.dataset.navProject === S.route.params.project));
  const fn = routes[S.route.view] || routes.home;
  const sheet = document.getElementById('sheet');
  try {
    await fn(sheet, S.route.params);
    // pages can add to a freshly drawn sheet (the first-run tips in guide.js)
    document.dispatchEvent(new CustomEvent('rendered', { detail: { sheet, view: routes[S.route.view] ? S.route.view : 'home' } }));
  } catch (e) {
    sheet.className = 'sheet';
    sheet.innerHTML = `<h1>Something went wrong</h1><p class="find err">${esc(String(e))}</p><p><button class="btn outline" data-act="reload">Try again</button></p>`;
  }
}

/** The tools the user shows (Apps > Show); hidden ones are left out of every page. */
export const shownTools = (st) => st.harnesses.filter((h) => !((st.settings && st.settings.hidden_tools) || []).includes(h.id));

/** Light, dark, or as Windows is (the stylesheet reads data-theme). */
export function applyTheme(st) {
  if (!st || !st.settings) return;
  document.documentElement.dataset.theme = st.settings.theme || 'system';
}

export async function loadState() {
  S.state = await call('state');
  applyTheme(S.state);
  const n = S.state.profiles.length, running = S.state.profiles.flatMap((p) => p.running.map((r) => ({ ...r, profile: p.name }))), live = running.length;
  document.getElementById('brand-sub').textContent = `${S.state.computer} · ${n} ${n === 1 ? 'account' : 'accounts'}`;
  setNavCounts({ 'n-accounts': n, 'n-live': live ? live + ' live' : '' });
  // the live strip in the top bar: one session opens straight away, several open the live list
  const strip = document.getElementById('live-strip');
  if (strip) {
    strip.hidden = !live;
    const one = live === 1 ? running[0] : null;
    strip.textContent = live ? `${live} live${one && one.cwd ? ' · ' + one.cwd.split(/[\\/]/).filter(Boolean).pop() : ''}` : '';
    strip.href = one && one.session_id ? `#session?profile=${encodeURIComponent(one.profile)}&id=${encodeURIComponent(one.session_id)}` : '#sessions?live=1';
  }
  setFresh();
  return S.state;
}

// ---------- dialogs ----------
export function openDialog(html, { onClose } = {}) {
  closeDialog();
  const root = document.getElementById('dialog-root');
  root.innerHTML = `<div class="dimA" data-act="dialog-close"></div><div class="dlg" role="dialog" aria-modal="true">${html}</div>`;
  root._onClose = onClose;
  const first = root.querySelector('input:not([type=hidden]),select,textarea,button.primary');
  if (first) first.focus();
  return root.querySelector('.dlg');
}

export function closeDialog() {
  const root = document.getElementById('dialog-root');
  if (root._onClose) { const f = root._onClose; root._onClose = null; f(); }
  root.innerHTML = '';
  // a background refresh that arrived while the dialog was open catches up now
  if (S.dirty) { S.dirty = false; render(); }
}

export const dlgHead = (title) => `<div class="dh"><b>${esc(title)}</b><button class="iconbtn" data-act="dialog-close" aria-label="Close">${ic.x}</button></div>`;

/** Ask before something that cannot be undone. Resolves true when confirmed. */
export function confirmDialog(title, body, okLabel = 'Confirm', danger = true) {
  return new Promise((resolve) => {
    let done = false;
    openDialog(`${dlgHead(title)}<div class="df" style="grid-template-columns:1fr"><div>${body}</div></div>
      <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn ${danger ? 'danger' : 'primary'}" id="confirm-ok">${esc(okLabel)}</button></span></div>`,
    { onClose: () => { if (!done) resolve(false); } });
    document.getElementById('confirm-ok').onclick = () => { done = true; closeDialog(); resolve(true); };
  });
}

// ---------- menus ----------
export function openMenu(anchor, items) {
  closeMenu();
  const r = anchor.getBoundingClientRect();
  const m = document.createElement('div');
  m.className = 'menu';
  m.innerHTML = items.map((it, i) => `<a data-i="${i}" class="${it.danger ? 'danger' : ''}">${esc(it.label)}</a>`).join('');
  document.body.appendChild(m);
  const left = Math.min(r.left, window.innerWidth - m.offsetWidth - 8);
  m.style.left = left + 'px';
  // near the bottom of the window the menu opens upward, so every item stays reachable
  m.style.top = (r.bottom + 4 + m.offsetHeight > window.innerHeight - 8 ? Math.max(8, r.top - 4 - m.offsetHeight) : r.bottom + 4) + 'px';
  m.onclick = (e) => {
    const a = e.target.closest('a[data-i]');
    if (a) { closeMenu(); items[+a.dataset.i].run(); }
  };
  setTimeout(() => {
    outside = (e) => { if (!m.contains(e.target)) closeMenu(); };
    document.addEventListener('click', outside);
  }, 0);
}
let outside = null;
export function closeMenu() {
  if (outside) { document.removeEventListener('click', outside); outside = null; }
  document.querySelectorAll('.menu').forEach((m) => m.remove());
}

// ---------- actions by data-act ----------
const actions = {
  'dialog-close': () => closeDialog(),
  'notice-close': () => { document.getElementById('notice').className = 'off'; },
  reload: () => render(),
};
export function action(name, fn) { actions[name] = fn; }

document.addEventListener('click', (e) => {
  const el = e.target.closest('[data-act]');
  if (!el) return;
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

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && document.getElementById('dialog-root').innerHTML) closeDialog();
  if (e.key === 'k' && e.ctrlKey) { e.preventDefault(); document.getElementById('find').focus(); }
  // a non-button element acting as one (role="button") answers Enter and Space like a button
  const rb = e.target.closest && e.target.closest('[role=button][data-act]');
  if (rb && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); rb.click(); }
});

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
