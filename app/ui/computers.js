// Across computers: the computers chip in the top bar, the "All computers · This computer" choice, and the helpers
// every page uses to draw per-computer data (dots, split bars, names). The operations (computers_status,
// computers_now, computers_set and the per-computer fields of home, sessions, projects and live) come from the sync
// core; until it is there, an "unknown operation" answer reads as "sync off, no peers", so every page draws as today.

import { S, call, action, render } from './core.js';
import { esc, dur } from './lib.js';

const OFF = { mode: 'off', folder: '', this: null, peers: [], last_run: null, error: null };
const unknownOp = (e) => /unknown operation/i.test(String(e));

/** computers_status, or the "off" state while the sync core is not there; kept in S.cache.computers for the page. */
export async function computersStatus() {
  let c;
  try { c = await call('computers_status'); } catch (e) { if (!unknownOp(e)) throw e; c = OFF; }
  c = { ...OFF, ...c, peers: c.peers || [], at: Date.now() };
  c.this = { id: '', name: (S.state && S.state.computer) || 'this computer', ...(c.this || {}) };
  S.cache.computers = c;
  return c;
}
export const syncOn = (c) => !!c && c.mode !== 'off';
/** Every computer, this one first. */
export const computers = (c) => (c ? [c.this, ...c.peers] : []);
export const nameOf = (c, id) => (computers(c).find((d) => d.id === id) || {}).name || id || '';
/** A computer's colour follows its place in the list: this one teal, then sand, iris, amber. */
const colorIdx = (c, id) => Math.max(0, computers(c).findIndex((d) => d.id === id)) % 4;
export const cdot = (c, id, title) => `<i class="cdot c${colorIdx(c, id)}" title="${esc(title || nameOf(c, id))}"></i>`;
export const cdots = (c, ids) => `<span class="cdots">${[...new Set(ids)].map((id) => cdot(c, id)).join('')}</span>`;
/** A value split by computer ([{id, name, value}]): a thin bar in the computers' colours, and the largest named. */
export function splitBar(c, parts) {
  const total = parts.reduce((a, p) => a + (p.value || 0), 0);
  if (parts.length < 2 || !total) return { bar: '', note: '' };
  const top = parts.reduce((a, p) => (p.value > a.value ? p : a), parts[0]);
  return { bar: `<div class="split" aria-hidden="true">${parts.map((p) => `<i class="c${colorIdx(c, p.id)}" style="width:${((p.value || 0) / total) * 100}%" title="${esc(p.name || nameOf(c, p.id))}"></i>`).join('')}</div>`,
    note: `${esc(top.name || nameOf(c, top.id))} ${Math.round((top.value / total) * 100)}%` };
}
/** The split a home tile carries (tiles[*].by_device), by the tile's key. */
export function tileSplit(d, key) {
  const t = d && (d.tiles || (d.dash && d.dash.tiles)), x = !t ? null : Array.isArray(t) ? t.find((v) => (v.key || v.id || v.metric) === key) : t[key];
  return (x && x.by_device) || [];
}

// ---------- All computers · This computer, remembered on this computer ----------
const SCOPE_KEY = 'hm.scope';
export const scope = () => { try { return localStorage.getItem(SCOPE_KEY) === 'here' ? 'here' : 'all'; } catch { return 'all'; } };
export const setScope = (v) => { try { localStorage.setItem(SCOPE_KEY, v); } catch {} };
export const scopeSeg = () => `<span class="seg3" role="group" aria-label="Computers" data-testid="scope">${[['all', 'All computers'], ['here', 'This computer']].map(([v, l]) => `<a class="${scope() === v ? 'on' : ''}" data-act="scope" data-v="${v}" data-testid="scope-${v}">${l}</a>`).join('')}</span>`;
action('scope', (el) => { setScope(el.dataset.v); render(); });

// ---------- the chip in the top bar ----------
const ago = (t) => (Date.now() - t < 60000 ? 'just now' : dur(Date.now() - t) + ' ago');
/** One dot per computer, teal when heard from within three minutes, and when the last sync round ran; hidden with sync off. */
export function drawChip(c) {
  const el = document.getElementById('pcs');
  if (!el) return;
  el.hidden = !syncOn(c);
  if (el.hidden) return;
  const now = Date.now();
  el.innerHTML = computers(c).map((d) => `<i class="cdot ${d === c.this || (d.last_seen && now - d.last_seen < 180000) ? 'on' : ''}" title="${esc(d.name)}${d.last_seen ? ' · heard ' + ago(d.last_seen) : ''}"></i>${esc(d.name)}`).join(' ')
    + `<span>${c.last_run ? 'synced ' + ago(c.last_run) : 'not synced yet'}</span>`;
}
// after a page is drawn, and once a minute: the status the page read, or a fresh one
document.addEventListener('rendered', () => { const c = S.cache.computers; (c && Date.now() - c.at < 5000 ? Promise.resolve(c) : computersStatus()).then(drawChip).catch(() => {}); });
setInterval(() => { if (document.visibilityState === 'visible') computersStatus().then(drawChip).catch(() => {}); }, 60000);
