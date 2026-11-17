// Computers, a page of its own (1.10): this computer's name, how the computers sync, what travels, the direct
// connection, and a card per computer. The body is the section Settings carried until 1.9, moved here as it was;
// the chip, the helpers and the dialogs (pairing, the hub, the recovery key, the first-run choice) are in computers.js.

import { S, call, view, toast, attempt, mutate, changed, openDialog, closeDialog, dlgHead, confirmDialog, action, render, pickFolder, loadState, patch, dialogOpen, keyStore } from './core.js';
import { esc, dur } from './lib.js';
import { winTrack } from './accounts.js';
import { computersStatus, syncOn, computers, colorIdx, heardOn, transportLine, nameOf, syncChoices } from './computers.js';
import { info, row, sw, field, btn } from './settings.js';
import { pageHead, plural } from './parts.js';

const stamp = (t) => new Date(t).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' });
const ago = (t) => (Date.now() - t < 60000 ? 'just now' : dur(Date.now() - t) + ' ago');

view('computers', async (sheet) => {
  sheet.className = 'sheet';
  const st = await loadState();
  const c = await computersStatus();
  const ts = syncOn(c) ? await call('transcripts_status').catch(() => null) : null;
  // the status: how many computers share their sessions and through what, and which are connected directly
  const n = computers(c).length, direct = c.peers.filter((p) => p.connected).map((p) => p.name);
  const status = !syncOn(c) ? 'Sync is off; only this computer is shown.'
    : `${plural(n, 'computer shares its', 'computers share their')} sessions ${c.mode === 'hub' ? 'through a hub' : c.mode === 'folder' ? 'through a folder' : 'directly'}${direct.length && c.mode !== 'off' ? `; ${esc(direct.join(', '))} ${direct.length === 1 ? 'is' : 'are'} connected directly` : ''}.`;
  patch(sheet, `<div class="page-body b-page" data-b-page="computers">
    ${pageHead('computers', status, '<button class="btn primary" data-act="pair" data-testid="pair-head">Pair a computer…</button>')}
    <section class="panel-card" data-testid="computers"><div class="b-section-body">${computersSection(c, st.settings, st, ts)}</div></section>
  </div>`);
});

// ---------- this computer's name, how it syncs, what travels, and a card per computer ----------
function computersSection(c, s, st, ts) {
  const on = syncOn(c), now = Date.now(), n = computers(c).length;
  const hub = c.hub || {}, hubOn = on && c.mode === 'hub';
  const card = (d) => {
    const me = d === c.this, seq = d.last_seq || 0, done = me ? (d.published_seq ?? seq) : (d.applied_seq || 0), pct = seq ? Math.min(100, Math.round((done / seq) * 100)) : 100;
    const stale = !me && !heardOn(d, now);
    const heard = me ? 'writing now' : d.connected ? 'connected' : d.last_seen ? (now - d.last_seen < 90000 ? 'writing now' : 'heard ' + ago(d.last_seen)) : 'not heard from yet';
    const facts = [d.projects != null ? `${d.projects} ${d.projects === 1 ? 'project' : 'projects'}` : '', d.accounts != null ? `${d.accounts} ${d.accounts === 1 ? 'account' : 'accounts'}` : '', `${(d.running || []).length} running`].filter(Boolean);
    // a peer: how it is reached now, when it was paired and last pulled (the ⓘ), its address, and Forget
    const peer = me ? '' : `<span class="muted" data-testid="transport">${esc(transportLine(c, d))} ${info('peer-' + d.id, `${d.direct ? `<b>Paired</b> ${d.paired_at ? esc(stamp(d.paired_at)) : 'yes'}` : '<b>Not paired</b>: known through the folder only'}<br><b>Last pull</b> ${d.last_pull ? esc(ago(d.last_pull)) : 'never'}${d.address ? `<br><b>Address</b> ${esc(d.address)}` : ''}${d.relayed_by ? `<br><b>Relayed through</b> ${esc(nameOf(c, d.relayed_by))}` : ''}`)}</span>
      <div class="page-actions">${d.direct ? `${field('addr-' + esc(d.id), 'Address', `class="mono" value="${esc(d.address || '')}" placeholder="name:port" maxlength="120" title="Where to reach ${esc(d.name)} when not on this network: a VPN name, with the port" data-testid="addr-${esc(d.id)}"`)}${btn('Save', 'peer-addr', `data-id="${esc(d.id)}" data-testid="addr-save-${esc(d.id)}"`)}` : ''}${btn('Forget…', 'peer-forget', `data-id="${esc(d.id)}" data-name="${esc(d.name)}" data-testid="forget-${esc(d.id)}"`)}</div>`;
    return `<article class="b-computer" data-key="computer-${esc(d.id || 'this')}" data-testid="computer-${esc(d.id || 'this')}"><div class="b-name"><i class="pc-dot c${colorIdx(c, d.id)}"></i><b>${esc(d.name)}</b>${me ? '<span class="plan-pill">This one</span>' : ''}</div>
      <span class="${stale ? 'b-warn' : 'muted'}">${esc([me ? { windows: 'Windows', macos: 'macOS', linux: 'Linux' }[st.os] || st.os : d.os, me ? st.version : d.app_version, heard].filter(Boolean).join(' · '))}</span>
      <div role="progressbar" aria-label="${esc(d.name)} entries ${me ? 'published' : 'applied'}" aria-valuenow="${done}" aria-valuemax="${Math.max(1, seq)}" aria-valuemin="0">${winTrack(pct, pct < 100 && stale ? 'warn' : '')}</div>
      <span>${done.toLocaleString()} of ${seq.toLocaleString()} entries ${me ? 'published' : 'applied here'}</span><span class="muted">${esc(facts.join(' · '))}</span>${peer}</article>`;
  };
  return row('devname', 'This computer', info('devname', 'The name the other computers show for this one, in their lists, on sessions and on the chip at the top.<br><b>Default: the computer name</b> Windows or macOS gives it.'), 'The name your other computers see.',
      `${field('st-devname', 'Name', `value="${esc(c.this.name)}" maxlength="40" data-testid="st-devname"`)}${btn('Rename', 'computers-set', 'data-key="device_name" data-from="st-devname" data-testid="st-devname-save"')}`)
    + row('syncmode', 'How your computers sync', info('syncmode', 'Pick one per computer. Each computer writes its own journal there and reads the others\'; keys and logins never go in it. Switching later keeps everything, because the journal is the same either way.<br><b>Default: off.</b>'), 'Pick one transport; changing it keeps merged history.', syncChoices(c))
    + (hubOn || hub.ring ? row('ring', 'Recovery key', info('ring', 'The ring key seals every chunk, presence report and transcript before it goes to the hub, so the hub stores what it cannot read. It is made on the first computer that joins the hub, kept in ' + esc(keyStore()) + ', carried to paired computers with the pairing, and shown once as the recovery key: 8 groups of 4 letters and digits for your password manager. A computer that is not paired needs it typed in.<br><b>Default: made on the first computer.</b>'), hub.ring ? 'Stored on this computer; paired computers receive it with the pairing. Keep it in your password manager for a computer that is not paired, or a fresh install.' : 'No ring key: this computer cannot read or write the hub. Enter the recovery key shown on the computer that first joined, or pair with it.',
      `${hub.ring ? btn('Show again…', 'ring-show', 'data-testid="ring-show"') : ''}<button class="${hub.ring ? 'btn-secondary' : 'btn-primary'}" data-act="ring-enter" data-testid="ring-enter">Enter recovery key…</button>`) : '')
    + row('transcripts', 'Carry transcripts', info('transcripts', `Also share this computer's session transcripts, so they can be read and resumed on another computer. A transcript over 20 MB is not carried; the folder holds up to 200 MB of this computer's transcripts, the oldest going first.${ts ? `<br>Carried ${ts.carried} · ${(ts.bytes / 1048576).toFixed(1)} MB${ts.skipped && ts.skipped.length ? ` · ${ts.skipped.length} skipped` : ''}` : ''}<br><b>Default: on.</b>`), `${ts ? `Carried ${ts.carried} · ${(ts.bytes / 1048576).toFixed(1)} MB · ${(ts.skipped || []).length} skipped. ` : ''}Over 20 MB is skipped; the store holds 200 MB.`, sw('sync_transcripts', s.sync_transcripts !== false, 'Carry transcripts', 'st-transcripts', 'computers-set'))
    + row('presence', 'Presence', info('presence', 'Writes what is running here every minute while the app is open, so the other computers show it in Running now.<br><b>Default: on.</b>'), 'Share what runs here every minute.', sw('sync_presence', s.sync_presence !== false, 'Share what runs here', 'st-presence', 'computers-set'))
    + row('direct', 'Direct connection', info('direct', 'Finds your other computers on this network (mDNS) or at an address you give (a VPN name), after pairing once with a six-digit code, and talks to them over TLS: what runs there every ten seconds, their history the moment it is written. The folder keeps working underneath; a computer reached neither way is still merged through it.<br><b>Default: off.</b>'), 'Paired computers talk over TLS. Presence updates every 10 seconds; the folder keeps working.',
      `${sw('direct_connection', s.direct_connection, 'Connect directly', 'st-direct', 'computers-set')}${btn('Pair a computer…', 'pair', 'data-testid="pair"')}<button class="btn-secondary" data-act="listen-port" data-testid="listen-port-change">Listen port: <span data-testid="listen-port">${c.port || 'chosen at first use'}</span>…</button>`)
    + `<div class="b-toolbar"><b>${n} ${n === 1 ? 'computer' : 'computers'}</b><span class="muted">${on ? `Synced through a ${c.mode === 'hub' ? 'hub' : 'folder'}${c.last_run ? ' · last round ' + ago(c.last_run) : ''}` : 'Sync is off: only this computer'}</span>${on ? btn('Sync now', 'computers-now', 'data-testid="computers-now"') : ''}${btn('First-run choices…', 'firstrun-open')}</div>
      ${c.error ? `<div role="alert" class="notice-bar warn" data-testid="sync-error">${esc(c.error)}</div>` : ''}
      <div class="b-computers" data-testid="computer-cards">${computers(c).map(card).join('')}</div>`;
}
// a switch or the name: saved through computers_set, which answers with the new status
action('computers-set', async (el) => {
  const value = el.type === 'checkbox' ? el.checked : document.getElementById(el.dataset.from).value.trim();
  if (el.type !== 'checkbox' && !value) return toast('Enter a computer name.', true);
  if (await mutate(() => call('computers_set', { [el.dataset.key]: value }), 'Saved') === undefined && el.type === 'checkbox') el.checked = !el.checked;
});
action('sync-folder-pick', async () => {
  const p = await pickFolder();
  if (p && await mutate(() => call('computers_set', { sync_mode: 'folder', sync_folder: p }), 'Syncing through ' + p) !== undefined) (await import('./restore.js')).welcomeBack(p);
});
action('sync-off', async () => { if (await confirmDialog('Stop syncing', 'Leave the folder? Merged history stays on this computer, and the folder keeps what was written.', 'Leave')) mutate(() => call('computers_set', { sync_mode: 'off' }), 'Sync is off; the folder keeps what was written'); });
action('computers-now', async () => {
  const r = await attempt(() => call('computers_now'));
  if (!r) return;
  const n = (r.peers || []).length;
  toast(r.error ? 'Sync round: ' + r.error : `Synced with ${n} other ${n === 1 ? 'computer' : 'computers'}`, !!r.error);
  changed();
});
// the port the direct connection listens on, a peer's address, and forgetting a peer
action('listen-port', () => {
  openDialog(`${dlgHead('Listen port')}<div class="form-field"><label for="lp-v">Port</label><input class="mono" id="lp-v" type="number" min="0" max="65535" value="${(S.cache.computers || {}).port || 0}" data-testid="lp-v"><span class="muted">0 picks a free port once and keeps it. Your other computers find this one by mDNS or by its address with this port.</span></div>
    <div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button><button class="btn-primary" id="lp-ok" data-testid="lp-ok">Save</button></span></div>`, { size: 'sm', label: 'Listen port' });
  document.getElementById('lp-ok').onclick = async () => {
    const v = +document.getElementById('lp-v').value;
    if (!(v >= 0 && v <= 65535)) return toast('A port is 0 to 65535', true);
    closeDialog();
    mutate(() => call('computers_set', { listen_port: v }), 'Saved');
  };
});
action('peer-addr', (el) => mutate(() => call('computers_set_peer', { id: el.dataset.id, address: document.getElementById('addr-' + el.dataset.id).value.trim() }), 'Saved'));
action('peer-forget', async (el) => {
  if (await confirmDialog('Forget ' + el.dataset.name, `Forget <b>${esc(el.dataset.name)}</b>? This computer stops connecting to it and drops its certificate; what was merged from it stays. Pair again to reconnect.`, 'Forget')) mutate(() => call('computers_forget', { id: el.dataset.id }), 'Forgotten');
});
// presence arrives every 10 s while a peer is connected: the page follows in place (the reconciler keeps what is being edited)
document.addEventListener('presence', () => { if (S.route.view === 'computers' && S.state && !dialogOpen()) render(true); });
