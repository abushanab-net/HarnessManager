// Across computers: the computers chip in the top bar, the "All computers · This computer" choice, and the helpers
// every page uses to draw per-computer data (dots, split bars, names). The operations (computers_status,
// computers_now, computers_set and the per-computer fields of home, sessions, projects and live) come from the sync
// core; until it is there, an "unknown operation" answer reads as "sync off, no peers", so every page draws as today.
// Phase 2 adds the direct connection: the pairing dialog (computers_pair_start | cancel | join and the
// computers-pairing event), the transport line of a computer card, and a 10 s tick while a peer is connected.
// Phase 5 adds the hub: Join with a code…, Invite a computer…, the recovery key dialogs (hub_join | hub_invite |
// ring_key_show | ring_key_enter), "hub · connected" and "rendezvous" on the cards, and the first-run choice.

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
  el.innerHTML = computers(c).map((d) => `<i class="cdot ${d === c.this || heardOn(d, now) ? 'on' : ''}" title="${esc(d.name)}${d.last_seen ? ' · heard ' + ago(d.last_seen) : ''}"></i>${esc(d.name)}`).join(' ')
    + `<span>${c.last_run ? 'synced ' + ago(c.last_run) : 'not synced yet'}</span>`;
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

// ---------- pairing: a six-digit code shown on one computer, typed on the other ----------
/** "482915" -> "482 915": digits only, six at most. */
export const fmtCode = (v) => String(v).replace(/\D/g, '').slice(0, 6).replace(/^(\d{3})(\d)/, '$1 $2');
const mmss = (ms) => `${Math.max(0, Math.floor(ms / 60000))}:${String(Math.max(0, Math.floor((ms % 60000) / 1000))).padStart(2, '0')}`;
let pairing = null; // { code, timer } while a code is shown here
const errText = (e) => String(e).replace(/^Error: /, '');
/** The dialog, on the Show a code or the Enter a code tab. */
export function pairDialog(tab = 'show') {
  const body = tab === 'show'
    ? `<div class="df" style="grid-template-columns:1fr"><div id="pair-show-body" data-testid="pair-show-body"><p class="muted">Asking for a code…</p></div></div>
      <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close" data-testid="pair-cancel">Cancel</button></span></div>`
    : `<div class="df"><label for="pair-code-in">Code</label><div><input class="input mono" id="pair-code-in" inputmode="numeric" autocomplete="off" placeholder="482 915" maxlength="7" aria-label="The six-digit code" data-testid="pair-code-in"><span class="hint">The six digits shown on the other computer, under Pair a computer… › Show a code.</span></div></div>
      <div class="df"><label for="pair-host">Address</label><div><input class="input mono" id="pair-host" placeholder="name:port" autocomplete="off" aria-label="Address of the other computer" data-testid="pair-host"><span class="hint">Only for a computer not on this network: the address shown under its code (a VPN name and port).</span></div></div>
      <div class="df" style="grid-template-columns:1fr"><p class="err" id="pair-error" data-testid="pair-error" hidden></p></div>
      <div class="dfoot"><span class="kb">Enter</span> pair <span class="kb">Esc</span> cancel<span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="pair-join" data-testid="pair-join">Pair</button></span></div>`;
  openDialog(`${dlgHead('Pair a computer')}
    <div class="dseg" data-testid="pair-tabs"><a class="${tab === 'show' ? 'on' : ''}" data-act="pair-tab" data-v="show" data-testid="pair-show">Show a code</a><a class="${tab === 'enter' ? 'on' : ''}" data-act="pair-tab" data-v="enter" data-testid="pair-enter">Enter a code</a></div>${body}`,
  { onClose: () => { if (pairing) { clearInterval(pairing.timer); pairing = null; call('computers_pair_cancel').catch(() => {}); } } });
  if (tab === 'show') showCode();
  else {
    const inp = document.getElementById('pair-code-in');
    inp.addEventListener('input', () => { inp.value = fmtCode(inp.value); });
    inp.addEventListener('keydown', (e) => { if (e.key === 'Enter') document.getElementById('pair-join').click(); });
    document.getElementById('pair-join').onclick = joinCode;
  }
}
async function showCode() {
  const el = document.getElementById('pair-show-body');
  let r;
  try { r = await call('computers_pair_start'); } catch (e) { if (el.isConnected) el.innerHTML = `<p class="err" data-testid="pair-error">${esc(errText(e))}</p>`; return; }
  if (!el.isConnected) { call('computers_pair_cancel').catch(() => {}); return; } // the dialog went while asking
  el.innerHTML = `<p>On the other computer, open Settings › Computers › Pair a computer… › Enter a code, and type</p>
    <div class="paircode mono" data-testid="pair-code">${esc(fmtCode(r.code))}</div>
    <p class="hint">Good for <span id="pair-left" class="mono" data-testid="pair-left">${mmss(r.expires_at - Date.now())}</span>. On another network, enter this address with the code: <span class="mono" data-testid="pair-host-hint">${esc(r.host_hint || '')}</span></p>`;
  const timer = setInterval(() => {
    const left = document.getElementById('pair-left');
    if (!left) return clearInterval(timer);
    const ms = r.expires_at - Date.now();
    if (ms > 0) return (left.textContent = mmss(ms));
    clearInterval(timer);
    if (pairing && pairing.timer === timer) pairing = null;
    el.innerHTML = `<p class="err" data-testid="pair-error">The code expired.</p><p><button class="btn" data-act="pair-tab" data-v="show">Show a new code</button></p>`;
  }, 1000);
  pairing = { code: r.code, timer };
}
async function joinCode() {
  const code = document.getElementById('pair-code-in').value.replace(/\D/g, ''), host = document.getElementById('pair-host').value.trim(), err = document.getElementById('pair-error');
  if (code.length !== 6) { err.textContent = 'Enter the six digits shown on the other computer.'; err.hidden = false; return; }
  const btn = document.getElementById('pair-join');
  btn.disabled = true; err.hidden = true;
  try {
    const st = await call('computers_pair_join', { code, ...(host ? { host } : {}) });
    const peer = ((st && st.peers) || []).reduce((a, p) => (!a || (p.paired_at || 0) > (a.paired_at || 0) ? p : a), null);
    closeDialog();
    toast(peer ? `Paired with ${peer.name}` : 'Paired');
    await changed(); // the new card is on the Settings page
    document.querySelector(peer ? `[data-testid="computer-${CSS.escape(peer.id)}"]` : '[data-testid=computer-cards]')?.scrollIntoView({ block: 'nearest' });
  } catch (e) { err.textContent = errText(e); err.hidden = false; if (btn.isConnected) btn.disabled = false; }
}
action('pair', () => pairDialog('show'));
action('pair-tab', (el) => pairDialog(el.dataset.v));
// the other computer joined with the code shown here (or the pairing failed): say so, then the page catches up
const ev = window.__TAURI__ && window.__TAURI__.event;
if (ev) ev.listen('computers-pairing', (e) => {
  const r = e.payload || {}, el = document.getElementById('pair-show-body');
  if (el && pairing) {
    clearInterval(pairing.timer); pairing = null;
    el.innerHTML = r.ok ? `<p class="ok" id="pair-done" data-testid="pair-done">Paired with ${esc((r.peer || {}).name || 'the other computer')}.</p>` : `<p class="err" data-testid="pair-error">${esc(r.error || 'The pairing failed.')}</p>`;
    if (r.ok) setTimeout(() => { if (document.getElementById('pair-done')) closeDialog(); }, 1500);
  } else if (r.ok) toast(`Paired with ${(r.peer || {}).name || 'another computer'}`);
  if (r.ok) changed();
});

// ---------- the hub (phase 5): joining with a code, inviting, the recovery key ----------
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
    <div class="df"><label for="hub-url">Address</label><div><input class="input mono" id="hub-url" value="${esc(h.url || '')}" placeholder="https://hub.example.com" autocomplete="off" aria-label="Hub address" data-testid="hub-url-in"><span class="hint">The address of the hub you host (deploy/hub in the repository sets one up in ten lines).</span></div></div>
    <div class="df"><label for="hub-code">Code</label><div><input class="input mono" id="hub-code" inputmode="numeric" autocomplete="off" placeholder="482 915" maxlength="7" aria-label="The six-digit code" data-testid="hub-code"><span class="hint">The six digits from another computer's Settings › Computers › Invite a computer….</span></div></div>
    <div class="df"><label for="hub-admin">Admin token</label><div><input class="input mono" id="hub-admin" type="password" autocomplete="off" aria-label="The hub's admin token" data-testid="hub-admin"><span class="hint">For the first computer only: the HUB_ADMIN_TOKEN of your hub. It makes the ring key, shown once as the recovery key.</span></div></div>
    <div class="df" style="grid-template-columns:1fr"><p class="err" id="hub-error" data-testid="hub-error" hidden></p></div>
    <div class="dfoot"><span class="kb">Enter</span> join <span class="kb">Esc</span> cancel<span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="hub-ok" data-testid="hub-ok">Join</button></span></div>`);
  const code = document.getElementById('hub-code');
  code.addEventListener('input', () => { code.value = fmtCode(code.value); });
  for (const id of ['hub-url', 'hub-code', 'hub-admin']) document.getElementById(id).addEventListener('keydown', (e) => { if (e.key === 'Enter') document.getElementById('hub-ok').click(); });
  document.getElementById('hub-ok').onclick = async () => {
    const url = document.getElementById('hub-url').value.trim(), c = code.value.replace(/\D/g, ''), admin = document.getElementById('hub-admin').value.trim(), err = document.getElementById('hub-error'), btn = document.getElementById('hub-ok');
    const fail = (t) => { err.textContent = t; err.hidden = false; if (btn.isConnected) btn.disabled = false; };
    if (!/^https?:\/\//.test(url)) return fail('The address starts with https:// (or http:// on a private network).');
    if (!admin && c.length !== 6) return fail('Enter the six digits from Invite a computer…, or the admin token for the first computer.');
    btn.disabled = true; err.hidden = true;
    try {
      const st = await call('hub_join', { url, ...(admin ? { admin_token: admin } : { code: c }) });
      closeDialog();
      toast(`Joined ${url}`);
      await changed();
      afterJoin(st);
    } catch (e) { fail(errText(e)); }
  };
}
/** Invite a computer…: a one-time six-digit code, good for ten minutes, with the address to enter it at. */
async function inviteDialog() {
  openDialog(`${dlgHead('Invite a computer')}<div class="df" style="grid-template-columns:1fr"><div id="invite-body" data-testid="invite-body"><p class="muted">Asking the hub for a code…</p></div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Close</button></span></div>`);
  const el = document.getElementById('invite-body');
  try {
    const r = await call('hub_invite');
    if (!el.isConnected) return;
    el.innerHTML = `<p>On the other computer, open Settings › Computers › A hub you host › Join with a code…, enter this address and type</p>
      <div class="paircode mono" data-testid="invite-code">${esc(fmtCode(r.code))}</div>
      <p class="hint">Good for ten minutes, for one computer. Address: <span class="mono" data-testid="invite-url">${esc(hubOf().url || '')}</span>. If that computer is not paired with this one, it will also ask for the recovery key.</p>`;
  } catch (e) { if (el.isConnected) el.innerHTML = `<p class="err" data-testid="hub-error">${esc(errText(e))}</p>`; }
}
/** The recovery key, shown once at the first hub use (and again from Settings behind a confirm). */
async function ringShowDialog(first = false) {
  openDialog(`${dlgHead('Your recovery key')}<div class="df" style="grid-template-columns:1fr"><div id="ring-body" data-testid="ring-body"><p class="muted">Reading…</p></div></div>
    <div class="dfoot"><span class="acts"><button class="btn primary" data-act="dialog-close" data-testid="ring-saved">I saved it in my password manager</button></span></div>`);
  const el = document.getElementById('ring-body');
  try {
    const r = await call('ring_key_show');
    if (!el.isConnected) return;
    el.innerHTML = `<p>${first ? 'This computer made the ring key that seals everything it sends to the hub. ' : ''}Keep this recovery key in your password manager: a computer that is not paired with this one needs it to read the hub, and so does a fresh install.</p>
      <div class="ringkey mono" data-testid="ring-key">${esc(r.key)}</div>
      <p class="hint">Paired computers get the key with the pairing, without typing. The hub never sees it.</p>`;
  } catch (e) { if (el.isConnected) el.innerHTML = `<p class="err" data-testid="hub-error">${esc(errText(e))}</p>`; }
}
/** Enter recovery key: the 8 groups of 4, typed or pasted; spaces, dashes and case do not matter. */
function ringEnterDialog() {
  openDialog(`${dlgHead('Enter the recovery key')}
    <div class="df"><label for="ring-in">Recovery key</label><div><input class="input mono" id="ring-in" autocomplete="off" placeholder="ABCD EFGH IJKL MNOP QRST UVWX YZ23 4567" aria-label="The recovery key" data-testid="ring-in"><span class="hint">The 8 groups of 4 shown on the computer that first turned the hub on (Settings › Computers › Recovery key › Show again). Without it this computer cannot read the hub.</span></div></div>
    <div class="df" style="grid-template-columns:1fr"><p class="err" id="ring-error" data-testid="ring-error" hidden></p></div>
    <div class="dfoot"><span class="kb">Enter</span> save <span class="kb">Esc</span> cancel<span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="ring-ok" data-testid="ring-ok">Save</button></span></div>`);
  const inp = document.getElementById('ring-in');
  inp.addEventListener('input', () => { inp.value = fmtRing(inp.value); });
  inp.addEventListener('keydown', (e) => { if (e.key === 'Enter') document.getElementById('ring-ok').click(); });
  document.getElementById('ring-ok').onclick = async () => {
    const err = document.getElementById('ring-error'), btn = document.getElementById('ring-ok');
    btn.disabled = true; err.hidden = true;
    try { await call('ring_key_enter', { key: inp.value }); closeDialog(); toast('The ring key is set'); changed(); }
    catch (e) { err.textContent = errText(e); err.hidden = false; if (btn.isConnected) btn.disabled = false; }
  };
}
action('hub-join', hubJoinDialog);
action('hub-invite', inviteDialog);
action('ring-enter', ringEnterDialog);
action('ring-show', async () => { if (await confirmDialog('Show the recovery key', 'Show the recovery key on screen? Anyone who reads it can read your hub.', 'Show', false)) ringShowDialog(); });
action('hub-leave', async () => {
  if (await confirmDialog('Stop syncing through the hub', 'Leave the hub? This computer\'s record and token go there; what was merged from the others stays here, and the ring key stays on this computer.', 'Leave')) mutate(() => call('hub_leave'), 'Left the hub');
});

// ---------- first run: "Add this computer to your others", with both cards ----------
const FIRSTRUN_KEY = 'hm.firstrun';
export function firstRunDialog() {
  openDialog(`${dlgHead('Add this computer to your others')}<div class="df" style="grid-template-columns:1fr"><div><p>How should your computers sync? You can change this later in Settings › Computers.</p>
    <div class="mcards"><div class="mcard" role="button" tabindex="0" data-act="firstrun-folder" data-testid="firstrun-folder"><i></i><div><b>A shared folder</b><span>Choose the folder you already sync between your computers. If another computer has used it, its history appears here at once.</span></div></div>
    <div class="mcard" role="button" tabindex="0" data-act="firstrun-hub" data-testid="firstrun-hub"><i></i><div><b>A hub you host</b><span>Enter the hub's address and the six-digit code shown on a computer already joined, or its admin token for the first computer.</span></div></div></div></div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close" data-testid="firstrun-later">Not now</button></span></div>`);
}
action('firstrun-folder', async () => { closeDialog(); const p = await pickFolder(); if (p) mutate(() => call('computers_set', { sync_mode: 'folder', sync_folder: p }), 'Syncing through ' + p); });
action('firstrun-hub', () => { closeDialog(); hubJoinDialog(); });
// on the first start (no accounts yet), once, after the page is drawn and when no tour is in front of it
document.addEventListener('rendered', () => {
  const st = S.state;
  if (!st || !st.first_run || !st.settings || st.settings.sync_mode !== 'off') return;
  try { if (localStorage.getItem(FIRSTRUN_KEY)) return; localStorage.setItem(FIRSTRUN_KEY, '1'); } catch { return; }
  if (!document.getElementById('dialog-root').innerHTML && !document.querySelector('.tour-pop')) firstRunDialog();
});
