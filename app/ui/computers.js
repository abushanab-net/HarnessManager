// Across computers: the computers chip in the top bar, the "All computers · This computer" choice, and the helpers
// every page uses to draw per-computer data (dots, split bars, names). The operations (computers_status,
// computers_now, computers_set and the per-computer fields of home, sessions, projects and live) come from the sync
// core; until it is there, an "unknown operation" answer reads as "sync off, no peers", so every page draws as today.
// The direct connection: the pairing dialog (computers_pair_start | cancel | join and the computers-pairing event),
// the transport line of a computer card, and a 10 s tick while a peer is connected. The hub: Join with a code…,
// Invite a computer…, the recovery key dialogs (hub_join | hub_invite | ring_key_show | ring_key_enter), and the
// first-run choice "Add this computer to your others". The dialogs are drawn as the reference draws them.

import { S, call, action, render, openDialog, closeDialog, dlgHead, toast, changed, confirmDialog, pickFolder, mutate, redrawLive } from './core.js';
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
  redrawLive();
  drawChip(c); // every fresh read redraws the chip, so a page's own read never leaves it behind a stale one
  return c;
}
// sync is on with a folder or a hub, and also with the direct connection alone (paired computers, no folder)
export const syncOn = (c) => !!c && (c.mode !== 'off' || (c.peers || []).length > 0 || !!(S.state && S.state.settings && S.state.settings.direct_connection));
/** Every computer, this one first. */
export const computers = (c) => (c ? [c.this, ...c.peers] : []);
export const nameOf = (c, id) => (computers(c).find((d) => d.id === id) || {}).name || id || '';
/** Connected directly, or heard from (through any transport) within three minutes. */
export const heardOn = (d, now) => !!(d.connected || (d.last_seen && now - d.last_seen < 180000));
/** How a peer is reached now: "direct · mDNS · connected", "direct · rendezvous · connected", "relayed through <name>",
 *  "hub · connected", "folder · heard 40 s ago"… */
export function transportLine(c, d) {
  if (d.connected) return `direct · ${d.path === 'rendezvous' ? 'rendezvous' : d.path === 'address' && d.address ? d.address : 'mDNS'} · connected`;
  if (d.path === 'relay' && d.relayed_by) return `relayed through ${nameOf(c, d.relayed_by)}`;
  const via = d.transport === 'hub' ? 'hub' : 'folder';
  if (d.direct && !heardOn(d, Date.now())) return `not reachable · ${via} only`;
  return `${via} · ${d.last_seen ? (via === 'hub' && heardOn(d, Date.now()) ? 'connected' : 'heard ' + ago(d.last_seen)) : 'not heard from yet'}`;
}
/** A computer's colour follows its place in the list: this one teal, then sand, iris, amber. */
export const colorIdx = (c, id) => Math.max(0, computers(c).findIndex((d) => d.id === id)) % 4;
export const cdot = (c, id, title, cls = '') => `<i class="cdot c${colorIdx(c, id)} ${cls}" title="${esc(title || nameOf(c, id))}"></i>`;
export const cdots = (c, ids) => `<span class="cdots">${[...new Set(ids)].map((id) => cdot(c, id)).join('')}</span>`;
/** A value split by computer ([{id, name, value}]): a thin bar in the computers' colours, and the largest named. */
export function splitBar(c, parts) {
  const total = parts.reduce((a, p) => a + (p.value || 0), 0);
  if (parts.length < 2 || !total) return { bar: '', note: '' };
  const top = parts.reduce((a, p) => (p.value > a.value ? p : a), parts[0]);
  let x = 0;
  return { bar: `<svg class="split-bar" aria-hidden="true">${parts.map((p) => { const w = ((p.value || 0) / total) * 100, r = `<rect class="c${colorIdx(c, p.id)}" x="${x.toFixed(2)}%" y="0" width="${w.toFixed(2)}%" height="100%"><title>${esc(p.name || nameOf(c, p.id))}</title></rect>`; x += w; return r; }).join('')}</svg>`,
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
export const scopeSeg = () => `<div class="chip-group" role="group" aria-label="Computers" data-testid="scope">${[['all', 'All computers'], ['here', 'This computer']].map(([v, l]) => `<button class="filter-chip ${scope() === v ? 'active' : ''}" aria-pressed="${scope() === v}" data-act="scope" data-v="${v}" data-testid="scope-${v}">${l}</button>`).join('')}</div>`;
action('scope', (el) => { setScope(el.dataset.v); render(); });

// ---------- the chip in the top bar ----------
const ago = (t) => (Date.now() - t < 60000 ? 'just now' : dur(Date.now() - t) + ' ago');
/** A dot per computer in its colour (the name and when it was heard on hover), dimmed when not heard from within three
 *  minutes, then "N computers" with the last sync round on hover; hidden with sync off. As the reference draws it. */
export function drawChip(c) {
  const el = document.getElementById('pcs');
  if (!el) return;
  el.hidden = !syncOn(c);
  if (el.hidden) return;
  const now = Date.now(), n = computers(c).length;
  el.innerHTML = computers(c).map((d) => cdot(c, d.id, `${d.name}${d === c.this ? ' · this computer' : d.last_seen ? ' · heard ' + ago(d.last_seen) : ' · not heard from yet'}`, d === c.this || heardOn(d, now) ? 'on' : '')).join('')
    + `<span title="${c.last_run ? 'synced ' + ago(c.last_run) : 'not synced yet'}">${n} ${n === 1 ? 'computer' : 'computers'}</span>`;
}
// after a page is drawn, and once a minute: the status the page read, or a fresh one
document.addEventListener('rendered', () => { const c = S.cache.computers; (c && Date.now() - c.at < 5000 ? Promise.resolve(c) : computersStatus()).then(drawChip).catch(() => {}); });
// every 10 s while the direct connection is on or a peer is connected (presence arrives every 10 s), else once a minute;
// a 'presence' event lets a page redraw what moved (Running now, the computer cards) without drawing itself again
let tick = 0;
const live = () => !!((S.state && S.state.settings && S.state.settings.direct_connection) || (S.cache.computers || OFF).peers.some((p) => p.connected));
setInterval(() => {
  if (document.visibilityState !== 'visible' || (++tick % 6 && !live())) return;
  computersStatus().then((c) => { drawChip(c); if (live()) document.dispatchEvent(new CustomEvent('presence', { detail: c })); }).catch(() => {});
}, 10000);

// ---------- the dialogs' parts, as the reference draws them ----------
const field = (id, label, extra = '', hint = '') => `<div class="form-field"><label for="${id}">${label}</label><input id="${id}" ${extra}>${hint ? `<span class="muted">${hint}</span>` : ''}</div>`;
const errLine = (id) => `<p class="b-danger" id="${id}" data-testid="${id}" role="alert" hidden></p>`;
const foot = (buttons) => `<div class="modal-foot"><span class="acts">${buttons}</span></div>`;
const cancel = '<button class="btn-secondary" data-act="dialog-close">Cancel</button>';
const errText = (e) => String(e).replace(/^Error: /, '');
const fail = (errId, btnId, t) => { const err = document.getElementById(errId), btn = document.getElementById(btnId); err.textContent = t; err.hidden = false; if (btn && btn.isConnected) btn.disabled = false; };
const onEnter = (ids, okId) => { for (const id of ids) document.getElementById(id).addEventListener('keydown', (e) => { if (e.key === 'Enter') document.getElementById(okId).click(); }); };

// ---------- pairing: a six-digit code shown on one computer, typed on the other ----------
/** "482915" -> "482 915": digits only, six at most. */
export const fmtCode = (v) => String(v).replace(/\D/g, '').slice(0, 6).replace(/^(\d{3})(\d)/, '$1 $2');
const mmss = (ms) => `${Math.max(0, Math.floor(ms / 60000))}:${String(Math.max(0, Math.floor((ms % 60000) / 1000))).padStart(2, '0')}`;
let pairing = null; // { code, timer } while a code is shown here
/** The dialog, on the Show a code or the Enter a code tab. */
export function pairDialog(tab = 'show') {
  const body = tab === 'show'
    ? `<div class="b-dialog-content" id="pair-show-body" data-testid="pair-show-body"><p class="muted">Asking for a code…</p></div>${foot(`<button class="btn-secondary" data-act="dialog-close" data-testid="pair-cancel">Cancel</button>`)}`
    : `${field('pair-code-in', 'Code', 'class="mono" inputmode="numeric" autocomplete="off" placeholder="482 915" maxlength="7" aria-label="The six-digit code" data-testid="pair-code-in"', 'The six digits shown on the other computer, under Pair a computer… › Show a code.')}
      ${field('pair-host', 'Address (another network)', 'class="mono" placeholder="name:port" autocomplete="off" aria-label="Address of the other computer" data-testid="pair-host"', 'Only for a computer not on this network: the address shown under its code (a VPN name and port).')}
      ${errLine('pair-error')}${foot(`${cancel}<button class="btn-primary" id="pair-join" data-testid="pair-join">Pair</button>`)}`;
  openDialog(`${dlgHead('Pair a computer')}
    <nav class="page-tabs" data-testid="pair-tabs" aria-label="Pairing"><button class="filter-chip ${tab === 'show' ? 'active' : ''}" aria-pressed="${tab === 'show'}" data-act="pair-tab" data-v="show" data-testid="pair-show">Show a code</button><button class="filter-chip ${tab === 'enter' ? 'active' : ''}" aria-pressed="${tab === 'enter'}" data-act="pair-tab" data-v="enter" data-testid="pair-enter">Enter a code</button></nav>${body}`,
  { onClose: () => { if (pairing) { clearInterval(pairing.timer); pairing = null; call('computers_pair_cancel').catch(() => {}); } }, size: 'sm', label: 'Pair a computer' });
  if (tab === 'show') showCode();
  else {
    const inp = document.getElementById('pair-code-in');
    inp.addEventListener('input', () => { inp.value = fmtCode(inp.value); });
    onEnter(['pair-code-in', 'pair-host'], 'pair-join');
    document.getElementById('pair-join').onclick = joinCode;
  }
}
async function showCode() {
  const el = document.getElementById('pair-show-body');
  let r;
  try { r = await call('computers_pair_start'); } catch (e) { if (el.isConnected) el.innerHTML = `<p class="b-danger" data-testid="pair-error">${esc(errText(e))}</p>`; return; }
  if (!el.isConnected) { call('computers_pair_cancel').catch(() => {}); return; } // the dialog went while asking
  el.innerHTML = `<span>On the other computer: Settings › Computers › Pair a computer… › Enter a code, and type</span>
    <strong class="b-pair-code" data-testid="pair-code">${esc(fmtCode(r.code))}</strong>
    <span class="muted">Good for <span id="pair-left" data-testid="pair-left">${mmss(r.expires_at - Date.now())}</span> · on another network, enter this address with the code:</span><span class="mono" data-testid="pair-host-hint">${esc(r.host_hint || '')}</span>`;
  const timer = setInterval(() => {
    const left = document.getElementById('pair-left');
    if (!left) return clearInterval(timer);
    const ms = r.expires_at - Date.now();
    if (ms > 0) return (left.textContent = mmss(ms));
    clearInterval(timer);
    if (pairing && pairing.timer === timer) pairing = null;
    el.innerHTML = `<strong class="b-pair-code" data-testid="pair-error">Code expired</strong><div class="page-actions"><button class="btn-secondary" data-act="pair-tab" data-v="show">Show a new code</button></div>`;
  }, 1000);
  pairing = { code: r.code, timer };
}
async function joinCode() {
  const code = document.getElementById('pair-code-in').value.replace(/\D/g, ''), host = document.getElementById('pair-host').value.trim();
  if (code.length !== 6) return fail('pair-error', 'pair-join', 'Enter the six digits shown on the other computer.');
  const btn = document.getElementById('pair-join');
  btn.disabled = true; document.getElementById('pair-error').hidden = true;
  try {
    const st = await call('computers_pair_join', { code, ...(host ? { host } : {}) });
    const peer = ((st && st.peers) || []).reduce((a, p) => (!a || (p.paired_at || 0) > (a.paired_at || 0) ? p : a), null);
    closeDialog();
    toast(peer ? `Paired with ${peer.name}` : 'Paired');
    await changed(); // the new card is on the Settings page
    document.querySelector(peer ? `[data-testid="computer-${CSS.escape(peer.id)}"]` : '[data-testid=computer-cards]')?.scrollIntoView({ block: 'nearest' });
  } catch (e) { fail('pair-error', 'pair-join', errText(e)); }
}
action('pair', () => pairDialog('show'));
action('pair-tab', (el) => pairDialog(el.dataset.v));
// the other computer joined with the code shown here (or the pairing failed): say so, then the page catches up
const ev = window.__TAURI__ && window.__TAURI__.event;
if (ev) ev.listen('computers-pairing', (e) => {
  const r = e.payload || {}, el = document.getElementById('pair-show-body');
  if (el && pairing) {
    clearInterval(pairing.timer); pairing = null;
    el.innerHTML = r.ok ? `<p class="b-good" id="pair-done" data-testid="pair-done">Paired with ${esc((r.peer || {}).name || 'the other computer')}.</p>` : `<p class="b-danger" data-testid="pair-error">${esc(r.error || 'The pairing failed.')}</p>`;
    if (r.ok) setTimeout(() => { if (document.getElementById('pair-done')) closeDialog(); }, 1500);
  } else if (r.ok) toast(`Paired with ${(r.peer || {}).name || 'another computer'}`);
  if (r.ok) changed();
});

// ---------- the hub: joining with a code, inviting, the recovery key ----------
const hubOf = () => ((S.cache.computers || {}).hub || {});
/** "ABCDEFGH…" -> "ABCD EFGH …": the recovery key as typed, in groups of four. */
const fmtRing = (v) => String(v).toUpperCase().replace(/[^A-Z0-9]/g, '').slice(0, 32).replace(/(.{4})(?=.)/g, '$1 ');
/** After a join: the recovery key once, on the computer that made the ring; the Enter dialog on one that has none. */
function afterJoin(st) {
  const h = (st && st.hub) || {};
  if (h.ring && !h.ring_shown) ringShowDialog(true);
  else if (h.joined && !h.ring) ringEnterDialog();
}
/** Join with a code…: the hub's address, and the code from another computer's Invite or the admin token for the first. */
export function hubJoinDialog() {
  const h = hubOf();
  openDialog(`${dlgHead('Join a hub')}
    ${field('hub-url', 'Address', `class="mono" type="url" value="${esc(h.url || '')}" placeholder="https://hub.example.com" autocomplete="off" aria-label="Hub address" data-testid="hub-url-in"`, 'The address of the hub you host (deploy/hub in the repository sets one up in ten lines).')}
    ${field('hub-code', 'Code', 'class="mono" inputmode="numeric" autocomplete="off" placeholder="482 915" maxlength="7" aria-label="The six-digit code" data-testid="hub-code"', 'Six digits from another computer\'s Settings › Computers › Invite a computer….')}
    ${field('hub-admin', 'Admin token (first computer)', 'class="mono" type="password" autocomplete="off" aria-label="The hub\'s admin token" data-testid="hub-admin"', 'For the first computer only: the HUB_ADMIN_TOKEN of your hub. It makes the ring key, shown once as the recovery key.')}
    ${errLine('hub-error')}${foot(`${cancel}<button class="btn-primary" id="hub-ok" data-testid="hub-ok">Join</button>`)}`, { size: 'sm', label: 'Join a hub' });
  const code = document.getElementById('hub-code');
  code.addEventListener('input', () => { code.value = fmtCode(code.value); });
  onEnter(['hub-url', 'hub-code', 'hub-admin'], 'hub-ok');
  document.getElementById('hub-ok').onclick = async () => {
    const url = document.getElementById('hub-url').value.trim(), c = code.value.replace(/\D/g, ''), admin = document.getElementById('hub-admin').value.trim();
    if (!/^https?:\/\//.test(url)) return fail('hub-error', 'hub-ok', 'The address starts with https:// (or http:// on a private network).');
    if (!admin && c.length !== 6) return fail('hub-error', 'hub-ok', 'Enter the six digits from Invite a computer…, or the admin token for the first computer.');
    document.getElementById('hub-ok').disabled = true; document.getElementById('hub-error').hidden = true;
    try {
      const st = await call('hub_join', { url, ...(admin ? { admin_token: admin } : { code: c }) });
      closeDialog();
      toast(`Joined ${url}`);
      await changed();
      afterJoin(st);
    } catch (e) { fail('hub-error', 'hub-ok', errText(e)); }
  };
}
/** Invite a computer…: a one-time six-digit code, good for ten minutes, with the address to enter it at. */
async function inviteDialog() {
  openDialog(`${dlgHead('Invite a computer')}<div class="b-dialog-content" id="invite-body" data-testid="invite-body"><p class="muted">Asking the hub for a code…</p></div>${foot('<button class="btn-secondary" data-act="dialog-close">Close</button>')}`, { size: 'sm', label: 'Invite a computer' });
  const el = document.getElementById('invite-body');
  try {
    const r = await call('hub_invite');
    if (!el.isConnected) return;
    el.innerHTML = `<strong class="b-pair-code" data-testid="invite-code">${esc(fmtCode(r.code))}</strong><span>Good for ten minutes, for one computer: on it, Settings › Computers › A hub you host › Join with a code…, with this address.</span>
      <span class="mono" data-testid="invite-url">${esc(hubOf().url || '')}</span><span class="muted">A computer not paired with this one also asks for your recovery key.</span>`;
  } catch (e) { if (el.isConnected) el.innerHTML = `<p class="b-danger" data-testid="hub-error">${esc(errText(e))}</p>`; }
}
/** The recovery key, shown once at the first hub use (and again from Settings behind a confirm). */
async function ringShowDialog(first = false) {
  openDialog(`${dlgHead('Your recovery key')}<div class="b-dialog-content" id="ring-body" data-testid="ring-body"><p class="muted">Reading…</p></div>${foot('<button class="btn-primary" data-act="dialog-close" data-testid="ring-saved">I saved it in my password manager</button>')}`, { size: 'sm', label: 'Your recovery key' });
  const el = document.getElementById('ring-body');
  try {
    const r = await call('ring_key_show');
    if (!el.isConnected) return;
    el.innerHTML = `<span>${first ? 'This computer made the ring key that seals everything it sends to the hub. ' : ''}Anyone who reads this key can read your hub. Keep it in your password manager: a computer not paired with this one needs it, and so does a fresh install.</span>
      <strong class="b-ring-key mono" data-testid="ring-key">${esc(r.key)}</strong><span class="muted">Paired computers get the key with the pairing, without typing. The hub never sees it.</span>`;
  } catch (e) { if (el.isConnected) el.innerHTML = `<p class="b-danger" data-testid="hub-error">${esc(errText(e))}</p>`; }
}
/** Enter recovery key: the 8 groups of 4, typed or pasted; spaces, dashes and case do not matter. */
function ringEnterDialog() {
  openDialog(`${dlgHead('Enter the recovery key')}
    ${field('ring-in', 'Recovery key', 'class="mono" autocomplete="off" placeholder="8 groups of 4" aria-label="The recovery key" data-testid="ring-in"', 'Case, spaces and dashes are ignored. It was shown on the computer that first turned the hub on (Settings › Computers › Recovery key › Show again…).')}
    ${errLine('ring-error')}${foot(`${cancel}<button class="btn-primary" id="ring-ok" data-testid="ring-ok">Save</button>`)}`, { size: 'sm', label: 'Enter the recovery key' });
  const inp = document.getElementById('ring-in');
  inp.addEventListener('input', () => { inp.value = fmtRing(inp.value); });
  onEnter(['ring-in'], 'ring-ok');
  document.getElementById('ring-ok').onclick = async () => {
    document.getElementById('ring-ok').disabled = true; document.getElementById('ring-error').hidden = true;
    try { await call('ring_key_enter', { key: inp.value }); closeDialog(); toast('The ring key is set'); changed(); }
    catch (e) { fail('ring-error', 'ring-ok', errText(e)); }
  };
}
action('hub-join', hubJoinDialog);
action('hub-invite', inviteDialog);
action('ring-enter', ringEnterDialog);
action('ring-show', async () => { if (await confirmDialog('Show the recovery key', 'Anyone who reads it can read your hub. Show it on screen?', 'Show', false)) ringShowDialog(); });
action('hub-leave', async () => {
  if (await confirmDialog('Stop syncing', 'Leave the hub? Merged history and the recovery key stay on this computer; this computer\'s record and token go from the hub.', 'Leave')) mutate(() => call('hub_leave'), 'Left the hub');
});

// ---------- the two ways to sync, as cards: on Settings › Computers, and in the first-run dialog ----------
/** The folder card and the hub card; `first` draws them for the first-run dialog, whose buttons pick a folder or open Join. */
export function syncChoices(c, first = false) {
  const hub = c.hub || {}, folderOn = c.mode === 'folder', hubOn = c.mode === 'hub' && !!hub.joined, pill = (t) => `<span class="plan-pill b-good" data-testid="hub-state">${t}</span>`;
  return `<div class="b-sync-grid">
    <div class="b-sync-choice ${folderOn ? 'active' : ''}" data-testid="sync-folder"><b>A shared folder ${folderOn ? '<span class="plan-pill b-good">Selected</span>' : ''}</b><span class="muted">OneDrive, Google Drive, an SMB share or a NAS. Nothing to run; the service carries the files.</span>
      <span class="mono b-command" title="${esc(c.folder || '')}" data-testid="sync-folder-path">${esc(c.folder || 'No folder chosen yet')}</span>
      <div class="page-actions">${first ? '<button class="btn-secondary" data-act="firstrun-folder" data-testid="firstrun-folder">Choose folder…</button>' : `<button class="btn-secondary" data-act="sync-folder-pick" data-testid="sync-folder-pick">${c.folder ? 'Change…' : 'Choose…'}</button>${folderOn ? '<button class="btn-secondary" data-act="sync-off" data-testid="sync-off">Stop syncing…</button>' : ''}`}</div></div>
    <div class="b-sync-choice ${hubOn ? 'active' : ''}" data-testid="sync-hub"><b>A hub you host ${hubOn ? pill(hub.ws ? 'Live' : 'Joined') : ''}</b><span class="muted">Your VPS with Docker, behind Caddy. Works from any network, no VPN needed, and gives live presence everywhere. Only your computers can read it: everything but their names is sealed with your ring key.</span>
      <span class="mono b-command" title="${esc(hub.url || '')}" data-testid="hub-url">${esc(hub.url || 'Not joined yet')}</span>
      <div class="page-actions">${first ? '<button class="btn-secondary" data-act="firstrun-hub" data-testid="firstrun-hub">Join with a code…</button>' : hubOn ? '<button class="btn-secondary" data-act="hub-invite" data-testid="hub-invite">Invite a computer…</button><button class="btn-secondary" data-act="hub-leave" data-testid="hub-leave">Stop syncing…</button>' : '<button class="btn-secondary" data-act="hub-join" data-testid="hub-join">Join with a code…</button>'}</div>
      ${hubOn ? `<span class="muted" data-testid="hub-line">Joined · ${hub.devices || 0} ${hub.devices === 1 ? 'computer' : 'computers'} · ${hub.ws ? 'live' : 'polling'}</span>${hub.ring ? '' : '<b class="b-danger">No ring key: enter the recovery key below.</b>'}` : ''}</div></div>`;
}

// ---------- first run: "Add this computer to your others", with both cards ----------
const FIRSTRUN_KEY = 'hm.firstrun';
export function firstRunDialog() {
  openDialog(`${dlgHead('Add this computer to your others')}<span>How should your computers sync? You can change this later in Settings › Computers.</span>${syncChoices(S.cache.computers || OFF, true)}
    ${foot('<button class="btn-secondary" data-act="dialog-close" data-testid="firstrun-later">Not now</button>')}`, { label: 'Add this computer to your others' });
}
action('firstrun-open', firstRunDialog);
action('firstrun-folder', async () => { closeDialog(); const p = await pickFolder(); if (p) mutate(() => call('computers_set', { sync_mode: 'folder', sync_folder: p }), 'Syncing through ' + p); });
action('firstrun-hub', () => { closeDialog(); hubJoinDialog(); });
// on the first start (no accounts yet), once, after the page is drawn and when no tour is in front of it
document.addEventListener('rendered', () => {
  const st = S.state;
  if (!st || !st.first_run || !st.settings || st.settings.sync_mode !== 'off') return;
  try { if (localStorage.getItem(FIRSTRUN_KEY)) return; localStorage.setItem(FIRSTRUN_KEY, '1'); } catch { return; }
  if (!document.getElementById('dialog-root').innerHTML && !document.querySelector('.tour-pop')) firstRunDialog();
});
